//! Hardware-level secure erase:
//!   • NVMe Sanitize (Crypto Erase / Block Erase) via IOCTL_STORAGE_PROTOCOL_COMMAND
//!   • ATA Security Erase capability detection + advisory for offline tools
//!
//! Both operations require the drive to be open by an administrator / root process.
//! The drive firmware performs the actual erase internally — no byte-by-byte overwrite loop.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NvmeSanitizeAction {
    /// Crypto Erase: the firmware destroys all media encryption keys,
    /// rendering all data permanently unreadable in nanoseconds.
    CryptoErase,
    /// Block Erase: the firmware erases every NAND block.
    /// Slower but works on drives without per-sector encryption.
    BlockErase,
}

/// Drive-level erase capabilities as reported by the hardware.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecureEraseCapability {
    /// NVMe Sanitize command is supported (Crypto or Block Erase).
    pub nvme_sanitize_crypto: bool,
    pub nvme_sanitize_block: bool,
    /// ATA Security Feature Set is supported.
    pub ata_secure_erase: bool,
    /// ATA Security Erase Enhanced variant is supported.
    pub ata_enhanced_erase: bool,
    /// True if ATA drive is in FROZEN state (erase blocked until power-cycle).
    pub ata_frozen: bool,
}

impl Default for SecureEraseCapability {
    fn default() -> Self {
        SecureEraseCapability {
            nvme_sanitize_crypto: false,
            nvme_sanitize_block: false,
            ata_secure_erase: false,
            ata_enhanced_erase: false,
            ata_frozen: false,
        }
    }
}

// ─── Public API ──────────────────────────────────────────────────────────────

/// Query the hardware capabilities for `path` (e.g. `\\.\PhysicalDrive1`).
pub fn query_capability(path: &str) -> anyhow::Result<SecureEraseCapability> {
    platform::query_capability(path)
}

/// Issue an NVMe Sanitize command to the drive at `path`.
/// The drive firmware performs the erase asynchronously; this call returns
/// immediately after the command is accepted.
pub fn nvme_sanitize(path: &str, action: NvmeSanitizeAction) -> anyhow::Result<()> {
    platform::nvme_sanitize(path, action)
}

/// Poll NVMe Sanitize status (0–100 %, or None if not in progress / complete).
pub fn nvme_sanitize_status(path: &str) -> anyhow::Result<Option<u8>> {
    platform::nvme_sanitize_status(path)
}

// ─── Windows ─────────────────────────────────────────────────────────────────

#[cfg(windows)]
mod platform {
    use super::{NvmeSanitizeAction, SecureEraseCapability};
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    // Raw Win32 declarations — avoids windows-sys feature-flag fragility
    #[link(name = "kernel32")]
    extern "system" {
        fn CreateFileW(
            lp_file_name: *const u16,
            dw_desired_access: u32,
            dw_share_mode: u32,
            lp_security_attributes: *const std::ffi::c_void,
            dw_creation_disposition: u32,
            dw_flags_and_attributes: u32,
            h_template_file: isize,
        ) -> isize;

        fn CloseHandle(h_object: isize) -> i32;
        fn GetLastError() -> u32;

        fn DeviceIoControl(
            h_device: isize,
            dw_io_control_code: u32,
            lp_in_buffer: *const std::ffi::c_void,
            n_in_buffer_size: u32,
            lp_out_buffer: *mut std::ffi::c_void,
            n_out_buffer_size: u32,
            lp_bytes_returned: *mut u32,
            lp_overlapped: *const std::ffi::c_void,
        ) -> i32;
    }

    const GENERIC_READ: u32 = 0x8000_0000;
    const GENERIC_WRITE: u32 = 0x4000_0000;
    const FILE_SHARE_READ: u32 = 0x1;
    const FILE_SHARE_WRITE: u32 = 0x2;
    const OPEN_EXISTING: u32 = 3;
    const INVALID_HANDLE_VALUE: isize = -1isize;

    // IOCTL_STORAGE_PROTOCOL_COMMAND
    // CTL_CODE(0x2d, 0x04F0, METHOD_BUFFERED, FILE_READ_ACCESS|FILE_WRITE_ACCESS)
    // = (0x2d<<16) | (3<<14) | (0x04F0<<2) | 0
    const IOCTL_STORAGE_PROTOCOL_COMMAND: u32 = 0x002DD3C0;

    // IOCTL_STORAGE_QUERY_PROPERTY  (for log page reads via protocol-specific)
    #[allow(dead_code)]
    const IOCTL_STORAGE_QUERY_PROPERTY: u32 = 0x002D1400;

    // NVMe opcodes
    const NVME_ADMIN_SANITIZE: u8 = 0x84;
    const NVME_ADMIN_GET_LOG_PAGE: u8 = 0x02;

    // STORAGE_PROTOCOL_TYPE: ProtocolTypeNvme = 3
    const PROTOCOL_TYPE_NVME: u32 = 3;

    // CommandSpecific: STORAGE_PROTOCOL_SPECIFIC_NVME_ADMIN_COMMAND = 1
    const NVME_ADMIN_CMD: u32 = 1;

    // ── Header for STORAGE_PROTOCOL_COMMAND (without trailing Command[] byte) ─

    // Total fixed fields: 19 × u32 = 76 bytes
    #[repr(C, packed)]
    struct ProtocolCmdHdr {
        version: u32,
        length: u32,
        protocol_type: u32,
        flags: u32,
        error_code: u32,
        command_length: u32,
        error_info_length: u32,
        data_to_device_transfer_length: u32,
        data_from_device_transfer_length: u32,
        time_out_value: u32,
        error_info_offset: u32,
        data_to_device_buffer_offset: u32,
        data_from_device_buffer_offset: u32,
        command_specific: u32,
        reserved0: u32,
        fixed_protocol_return_data: u32,
        reserved1: [u32; 3],
    }
    const HDR_SIZE: usize = std::mem::size_of::<ProtocolCmdHdr>();
    const NVME_CMD_SIZE: usize = 64; // NVMe command is always 64 bytes

    struct WinHandle(isize);
    impl WinHandle {
        fn open(path: &str, access: u32) -> Option<Self> {
            let wide: Vec<u16> = OsStr::new(path)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let h = unsafe {
                CreateFileW(
                    wide.as_ptr(),
                    access,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    std::ptr::null::<std::ffi::c_void>(),
                    OPEN_EXISTING,
                    0,
                    0,
                )
            };
            if h == INVALID_HANDLE_VALUE { None } else { Some(WinHandle(h)) }
        }
    }
    impl Drop for WinHandle {
        fn drop(&mut self) {
            unsafe { CloseHandle(self.0) };
        }
    }

    /// Build a protocol command buffer with a 64-byte NVMe command payload.
    fn build_proto_buf(nvme_cmd: [u8; NVME_CMD_SIZE], from_device_len: u32) -> Vec<u8> {
        let total = HDR_SIZE + NVME_CMD_SIZE + from_device_len as usize;
        let mut buf = vec![0u8; total];

        let hdr = unsafe { &mut *(buf.as_mut_ptr() as *mut ProtocolCmdHdr) };
        hdr.version = 1;
        hdr.length = HDR_SIZE as u32 + NVME_CMD_SIZE as u32;
        hdr.protocol_type = PROTOCOL_TYPE_NVME;
        hdr.flags = 0;
        hdr.command_length = NVME_CMD_SIZE as u32;
        hdr.error_info_length = 0;
        hdr.data_to_device_transfer_length = 0;
        hdr.data_from_device_transfer_length = from_device_len;
        hdr.time_out_value = 60;
        hdr.error_info_offset = HDR_SIZE as u32;
        hdr.data_to_device_buffer_offset = 0;
        hdr.data_from_device_buffer_offset = if from_device_len > 0 {
            (HDR_SIZE + NVME_CMD_SIZE) as u32
        } else {
            0
        };
        hdr.command_specific = NVME_ADMIN_CMD;
        hdr.reserved0 = 0;
        hdr.fixed_protocol_return_data = 0;

        buf[HDR_SIZE..HDR_SIZE + NVME_CMD_SIZE].copy_from_slice(&nvme_cmd);
        buf
    }

    fn issue_protocol_cmd(handle: isize, buf: &mut Vec<u8>) -> Result<(), u32> {
        let mut returned = 0u32;
        let ok = unsafe {
            DeviceIoControl(
                handle,
                IOCTL_STORAGE_PROTOCOL_COMMAND,
                buf.as_ptr() as *const std::ffi::c_void,
                buf.len() as u32,
                buf.as_mut_ptr() as *mut std::ffi::c_void,
                buf.len() as u32,
                &mut returned,
                std::ptr::null::<std::ffi::c_void>(),
            )
        };
        if ok == 0 { Err(unsafe { GetLastError() }) } else { Ok(()) }
    }

    // ── NVMe Identify / log page helpers ────────────────────────────────────

    /// Read the NVMe Sanitize Status log page (log ID 0x81, 512 bytes).
    fn read_sanitize_log(handle: isize) -> anyhow::Result<[u8; 512]> {
        // GET LOG PAGE command: opcode 0x02, NUMD = 127 (512 bytes = 128 DWORDs, NUMD = count-1)
        let mut cmd = [0u8; NVME_CMD_SIZE];
        cmd[0] = NVME_ADMIN_GET_LOG_PAGE;
        // CDW10: LID=0x81, NUMDL=127 (lower 16 bits of NUMD)
        let cdw10: u32 = 0x81 | (127u32 << 16);
        cmd[40..44].copy_from_slice(&cdw10.to_le_bytes());

        let mut buf = build_proto_buf(cmd, 512);
        issue_protocol_cmd(handle, &mut buf)
            .map_err(|e| anyhow::anyhow!("GET LOG PAGE failed (error {})", e))?;

        let mut log = [0u8; 512];
        let data_offset = HDR_SIZE + NVME_CMD_SIZE;
        if buf.len() >= data_offset + 512 {
            log.copy_from_slice(&buf[data_offset..data_offset + 512]);
        }
        Ok(log)
    }

    /// Read NVMe Identify Controller data (4096 bytes) to check SANICAP field.
    fn read_identify_controller(handle: isize) -> anyhow::Result<[u8; 4096]> {
        let mut cmd = [0u8; NVME_CMD_SIZE];
        cmd[0] = 0x06; // Identify
        // CDW10: CNS=0x01 (Identify Controller)
        cmd[40] = 0x01;

        let mut buf = build_proto_buf(cmd, 4096);
        issue_protocol_cmd(handle, &mut buf)
            .map_err(|e| anyhow::anyhow!("Identify Controller failed (error {})", e))?;

        let mut id = [0u8; 4096];
        let data_offset = HDR_SIZE + NVME_CMD_SIZE;
        if buf.len() >= data_offset + 4096 {
            id.copy_from_slice(&buf[data_offset..data_offset + 4096]);
        }
        Ok(id)
    }

    pub fn query_capability(path: &str) -> anyhow::Result<SecureEraseCapability> {
        let Some(h) = WinHandle::open(path, GENERIC_READ | GENERIC_WRITE) else {
            let err = unsafe { GetLastError() };
            if err == 5 {
                anyhow::bail!("Access denied. Run NitroShred as Administrator.");
            }
            anyhow::bail!("Cannot open {:?} (error {})", path, err);
        };

        let mut cap = SecureEraseCapability::default();

        // Try NVMe Identify Controller to read SANICAP (bytes 328-331)
        if let Ok(id) = read_identify_controller(h.0) {
            let sanicap = u32::from_le_bytes([id[328], id[329], id[330], id[331]]);
            // Bit 0: Crypto Erase, Bit 1: Block Erase, Bit 2: Overwrite
            cap.nvme_sanitize_crypto = (sanicap & 0x1) != 0;
            cap.nvme_sanitize_block = (sanicap & 0x2) != 0;
        }

        // ATA capability detection is not practical via STORAGE_PROTOCOL_COMMAND on Windows
        // without the ATA pass-through IOCTL; leave defaults (false).

        Ok(cap)
    }

    pub fn nvme_sanitize(path: &str, action: NvmeSanitizeAction) -> anyhow::Result<()> {
        let Some(h) = WinHandle::open(path, GENERIC_READ | GENERIC_WRITE) else {
            let err = unsafe { GetLastError() };
            if err == 5 {
                anyhow::bail!("Access denied. Run NitroShred as Administrator.");
            }
            anyhow::bail!("Cannot open {:?} (error {})", path, err);
        };

        // Sanitize action codes per NVMe spec 2.x
        // 001b = Exit Failure, 010b = Block Erase, 011b = Overwrite, 100b = Crypto Erase
        let sanact: u32 = match action {
            NvmeSanitizeAction::CryptoErase => 4,
            NvmeSanitizeAction::BlockErase => 2,
        };
        // AUSE = 1 (bit 3) allows the host to issue other commands while sanitize runs
        let cdw10: u32 = sanact | (1u32 << 3);

        let mut cmd = [0u8; NVME_CMD_SIZE];
        cmd[0] = NVME_ADMIN_SANITIZE;
        cmd[40..44].copy_from_slice(&cdw10.to_le_bytes());

        let mut buf = build_proto_buf(cmd, 0);
        issue_protocol_cmd(h.0, &mut buf).map_err(|e| {
            anyhow::anyhow!(
                "NVMe Sanitize command rejected (error {}). \
                 Drive may not support this action or may require a cold-boot.",
                e
            )
        })?;

        Ok(())
    }

    pub fn nvme_sanitize_status(path: &str) -> anyhow::Result<Option<u8>> {
        let Some(h) = WinHandle::open(path, GENERIC_READ | GENERIC_WRITE) else {
            anyhow::bail!("Cannot open {:?}", path);
        };

        let log = read_sanitize_log(h.0)?;

        // Sanitize Status log layout (NVMe spec):
        // Bytes 0-1: SPROG (progress 0-65535, 65535 = complete)
        // Bytes 2-3: SSTAT (bits[2:0] = status code)
        let sstat = u16::from_le_bytes([log[2], log[3]]);
        let status_code = sstat & 0x7;
        let sprog = u16::from_le_bytes([log[0], log[1]]);

        match status_code {
            0x0 => Ok(None),  // Never started
            0x1 => {          // In progress
                let pct = (sprog as u32 * 100 / 65535) as u8;
                Ok(Some(pct))
            }
            0x2 => Ok(Some(100)), // Completed successfully
            0x3 => anyhow::bail!("NVMe Sanitize failed (SSTAT=3)"),
            _ => Ok(None),
        }
    }
}

// ─── Linux ───────────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
mod platform {
    use super::{NvmeSanitizeAction, SecureEraseCapability};

    // Linux nvme-generic ioctl constants
    const NVME_IOCTL_ADMIN_CMD: u64 = 0xC0484E41;

    #[repr(C)]
    struct NvmeAdminCmd {
        opcode: u8,
        flags: u8,
        rsvd1: u16,
        nsid: u32,
        cdw2: u32,
        cdw3: u32,
        metadata: u64,
        addr: u64,
        metadata_len: u32,
        data_len: u32,
        cdw10: u32,
        cdw11: u32,
        cdw12: u32,
        cdw13: u32,
        cdw14: u32,
        cdw15: u32,
        timeout_ms: u32,
        result: u32,
    }

    pub fn query_capability(path: &str) -> anyhow::Result<SecureEraseCapability> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map_err(|e| anyhow::anyhow!("Cannot open {:?}: {}", path, e))?;

        let fd = std::os::unix::io::AsRawFd::as_raw_fd(&file);
        let mut id_data = vec![0u8; 4096];

        let mut cmd = NvmeAdminCmd {
            opcode: 0x06, // Identify
            flags: 0,
            rsvd1: 0,
            nsid: 0,
            cdw2: 0,
            cdw3: 0,
            metadata: 0,
            addr: id_data.as_mut_ptr() as u64,
            metadata_len: 0,
            data_len: 4096,
            cdw10: 0x01, // CNS = Identify Controller
            cdw11: 0,
            cdw12: 0,
            cdw13: 0,
            cdw14: 0,
            cdw15: 0,
            timeout_ms: 5000,
            result: 0,
        };

        let mut cap = SecureEraseCapability::default();

        let ret = unsafe { libc::ioctl(fd, NVME_IOCTL_ADMIN_CMD, &mut cmd) };
        if ret == 0 {
            let sanicap = u32::from_le_bytes([id_data[328], id_data[329], id_data[330], id_data[331]]);
            cap.nvme_sanitize_crypto = (sanicap & 0x1) != 0;
            cap.nvme_sanitize_block = (sanicap & 0x2) != 0;
        }

        Ok(cap)
    }

    pub fn nvme_sanitize(path: &str, action: NvmeSanitizeAction) -> anyhow::Result<()> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map_err(|e| anyhow::anyhow!("Cannot open {:?}: {}", path, e))?;

        let fd = std::os::unix::io::AsRawFd::as_raw_fd(&file);

        let sanact: u32 = match action {
            NvmeSanitizeAction::CryptoErase => 4,
            NvmeSanitizeAction::BlockErase => 2,
        };
        let cdw10 = sanact | (1u32 << 3); // AUSE = 1

        let mut cmd = NvmeAdminCmd {
            opcode: 0x84, // Sanitize
            flags: 0,
            rsvd1: 0,
            nsid: 0,
            cdw2: 0,
            cdw3: 0,
            metadata: 0,
            addr: 0,
            metadata_len: 0,
            data_len: 0,
            cdw10,
            cdw11: 0,
            cdw12: 0,
            cdw13: 0,
            cdw14: 0,
            cdw15: 0,
            timeout_ms: 5000,
            result: 0,
        };

        let ret = unsafe { libc::ioctl(fd, NVME_IOCTL_ADMIN_CMD, &mut cmd) };
        if ret != 0 {
            anyhow::bail!(
                "NVMe Sanitize ioctl failed (errno {}). Ensure device is not mounted and \
                 process has CAP_SYS_ADMIN.",
                std::io::Error::last_os_error()
            );
        }

        Ok(())
    }

    pub fn nvme_sanitize_status(path: &str) -> anyhow::Result<Option<u8>> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map_err(|e| anyhow::anyhow!("Cannot open {:?}: {}", path, e))?;

        let fd = std::os::unix::io::AsRawFd::as_raw_fd(&file);
        let mut log_data = vec![0u8; 512];

        // GET LOG PAGE: log ID 0x81 (Sanitize Status), 512 bytes
        let mut cmd = NvmeAdminCmd {
            opcode: 0x02,
            flags: 0,
            rsvd1: 0,
            nsid: 0xFFFF_FFFF,
            cdw2: 0,
            cdw3: 0,
            metadata: 0,
            addr: log_data.as_mut_ptr() as u64,
            metadata_len: 0,
            data_len: 512,
            cdw10: 0x81 | (127u32 << 16),
            cdw11: 0,
            cdw12: 0,
            cdw13: 0,
            cdw14: 0,
            cdw15: 0,
            timeout_ms: 5000,
            result: 0,
        };

        let ret = unsafe { libc::ioctl(fd, NVME_IOCTL_ADMIN_CMD, &mut cmd) };
        if ret != 0 {
            return Ok(None);
        }

        let sstat = u16::from_le_bytes([log_data[2], log_data[3]]);
        let status_code = sstat & 0x7;
        let sprog = u16::from_le_bytes([log_data[0], log_data[1]]);

        match status_code {
            0x0 => Ok(None),
            0x1 => Ok(Some((sprog as u32 * 100 / 65535) as u8)),
            0x2 => Ok(Some(100)),
            0x3 => anyhow::bail!("NVMe Sanitize reported failure"),
            _ => Ok(None),
        }
    }
}

// ─── Other platforms (stub) ──────────────────────────────────────────────────

#[cfg(not(any(windows, target_os = "linux")))]
mod platform {
    use super::{NvmeSanitizeAction, SecureEraseCapability};

    pub fn query_capability(_path: &str) -> anyhow::Result<SecureEraseCapability> {
        Ok(SecureEraseCapability::default())
    }

    pub fn nvme_sanitize(_path: &str, _action: NvmeSanitizeAction) -> anyhow::Result<()> {
        anyhow::bail!("NVMe Sanitize not supported on this platform");
    }

    pub fn nvme_sanitize_status(_path: &str) -> anyhow::Result<Option<u8>> {
        Ok(None)
    }
}
