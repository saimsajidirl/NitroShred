//! Raw sector-level wipe of physical drives (\\.\PhysicalDriveN / /dev/sdX).
//! Bypasses the filesystem entirely — equivalent to nwipe/DBAN zero-fill mode.

use serde::{Deserialize, Serialize};
use std::sync::{atomic::AtomicBool, Arc};

/// Information about a physical block device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicalDriveInfo {
    /// OS-level path: `\\.\PhysicalDrive0` or `/dev/sda`.
    pub path: String,
    /// Human-readable model string from the drive firmware.
    pub model: String,
    /// Drive serial number (may be empty).
    pub serial: String,
    /// Total raw capacity in bytes.
    pub size_bytes: u64,
    /// Logical sector size in bytes (512 or 4096).
    pub sector_size: u32,
    /// Bus / media type string: "NVMe", "SATA SSD", "HDD", "USB", "Unknown".
    pub media_type: String,
    /// True if this drive is removable media.
    pub is_removable: bool,
    /// True if this drive hosts the running OS (C:\\ on Windows, / on Linux).
    pub is_system: bool,
}

// ─── Public API ──────────────────────────────────────────────────────────────

pub fn list_physical_drives() -> anyhow::Result<Vec<PhysicalDriveInfo>> {
    platform::list_drives()
}

/// Zero-fill every sector of `path` (e.g. `\\.\PhysicalDrive1`).
/// `cancel` — set to true from another thread to abort early.
/// `progress(bytes_done, total_bytes, speed_mb_s)` — called periodically.
/// Returns total bytes written.
pub fn raw_wipe_physical_drive(
    path: &str,
    cancel: Arc<AtomicBool>,
    progress: impl Fn(u64, u64, f64) + Send + 'static,
) -> anyhow::Result<u64> {
    platform::raw_wipe(path, cancel, progress)
}

// ─── Windows ─────────────────────────────────────────────────────────────────

#[cfg(windows)]
mod platform {
    use super::PhysicalDriveInfo;
    use anyhow::Context;
    use std::alloc::{alloc_zeroed, dealloc, Layout};
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    use std::time::Instant;

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

        fn WriteFile(
            h_file: isize,
            lp_buffer: *const u8,
            n_number_of_bytes_to_write: u32,
            lp_number_of_bytes_written: *mut u32,
            lp_overlapped: *const std::ffi::c_void,
        ) -> i32;

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
    const FILE_FLAG_NO_BUFFERING: u32 = 0x2000_0000;
    const FILE_FLAG_WRITE_THROUGH: u32 = 0x8000_0000;
    const INVALID_HANDLE_VALUE: isize = -1isize;

    // IOCTL codes (hand-computed via CTL_CODE macro)
    const IOCTL_DISK_GET_DRIVE_GEOMETRY_EX: u32 = 0x000700A0;
    const IOCTL_STORAGE_QUERY_PROPERTY: u32 = 0x002D1400;
    const IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS: u32 = 0x00560000;

    const BUF_SIZE: usize = 4 * 1024 * 1024; // 4 MB
    const BUF_ALIGN: usize = 4096; // safe for both 512-byte and 4096-byte sector drives

    // ── Structures for IOCTLs ────────────────────────────────────────────────

    #[repr(C)]
    struct DiskGeometry {
        cylinders: i64,
        media_type: u32,
        tracks_per_cylinder: u32,
        sectors_per_track: u32,
        bytes_per_sector: u32,
    }

    #[repr(C)]
    struct DiskGeometryEx {
        geometry: DiskGeometry,
        disk_size: i64,
        _data: u8,
    }

    #[repr(C)]
    struct StoragePropertyQuery {
        property_id: u32, // 0 = StorageDeviceProperty
        query_type: u32,  // 0 = PropertyStandardQuery
        _additional: u8,
    }

    #[repr(C)]
    struct StorageDeviceDescriptorHdr {
        version: u32,
        size: u32,
        device_type: u8,
        device_type_modifier: u8,
        removable_media: u8,
        command_queueing: u8,
        vendor_id_offset: u32,
        product_id_offset: u32,
        product_revision_offset: u32,
        serial_number_offset: u32,
        bus_type: u32, // STORAGE_BUS_TYPE enum
        raw_properties_length: u32,
    }

    // STORAGE_BUS_TYPE values we care about
    const BUS_TYPE_SCSI: u32 = 0x1;
    const BUS_TYPE_ATAPI: u32 = 0x2;
    const BUS_TYPE_ATA: u32 = 0x3;
    const BUS_TYPE_USB: u32 = 0x7;
    const BUS_TYPE_NVME: u32 = 0x11;
    const BUS_TYPE_SATA: u32 = 0xB;

    #[repr(C)]
    struct DiskExtent {
        disk_number: u32,
        _padding: u32,
        starting_offset: i64,
        extent_length: i64,
    }

    #[repr(C)]
    struct VolumeDiskExtents {
        number_of_disk_extents: u32,
        extents: [DiskExtent; 8],
    }

    // ── Internal helpers ─────────────────────────────────────────────────────

    struct WinHandle(isize);
    impl WinHandle {
        /// Open a raw device path for IOCTL queries (read-only, no buffering flags).
        fn open_ioctl(path: &str) -> Option<Self> {
            Self::open(path, GENERIC_READ, 0)
        }
        fn open(path: &str, access: u32, flags: u32) -> Option<Self> {
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
                    flags,
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

    fn ioctl_in_out(handle: isize, code: u32, in_buf: *const std::ffi::c_void, in_len: u32,
                    out_buf: *mut std::ffi::c_void, out_len: u32) -> bool {
        let mut returned = 0u32;
        unsafe {
            DeviceIoControl(handle, code, in_buf, in_len, out_buf, out_len,
                            &mut returned, std::ptr::null::<std::ffi::c_void>()) != 0
        }
    }

    fn geometry_ex(handle: isize) -> Option<(u64, u32)> {
        let mut geo = unsafe { std::mem::zeroed::<DiskGeometryEx>() };
        let ok = ioctl_in_out(
            handle, IOCTL_DISK_GET_DRIVE_GEOMETRY_EX,
            std::ptr::null::<std::ffi::c_void>(), 0,
            &mut geo as *mut _ as *mut _,
            std::mem::size_of::<DiskGeometryEx>() as u32,
        );
        if ok && geo.disk_size > 0 {
            Some((geo.disk_size as u64, geo.geometry.bytes_per_sector.max(512)))
        } else {
            None
        }
    }

    fn device_descriptor(handle: isize) -> (String, String, bool, u32) {
        let mut buf = vec![0u8; 512];
        let query = StoragePropertyQuery {
            property_id: 0,
            query_type: 0,
            _additional: 0,
        };
        let ok = ioctl_in_out(
            handle, IOCTL_STORAGE_QUERY_PROPERTY,
            &query as *const _ as *const _,
            std::mem::size_of::<StoragePropertyQuery>() as u32,
            buf.as_mut_ptr() as *mut _,
            buf.len() as u32,
        );
        let returned = buf.iter().rposition(|&b| b != 0).unwrap_or(0) as u32 + 1;
        if !ok || returned < std::mem::size_of::<StorageDeviceDescriptorHdr>() as u32 {
            return ("Unknown".into(), String::new(), false, 0);
        }

        let hdr = unsafe { &*(buf.as_ptr() as *const StorageDeviceDescriptorHdr) };
        let removable = hdr.removable_media != 0;
        let bus_type = hdr.bus_type;

        let read_str = |offset: u32| -> String {
            if offset == 0 || offset as usize >= buf.len() {
                return String::new();
            }
            let slice = &buf[offset as usize..];
            let end = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
            String::from_utf8_lossy(&slice[..end]).trim().to_owned()
        };

        let vendor = read_str(hdr.vendor_id_offset);
        let product = read_str(hdr.product_id_offset);
        let serial = read_str(hdr.serial_number_offset);

        let model = match (vendor.is_empty(), product.is_empty()) {
            (true, true) => "Unknown".to_owned(),
            (true, false) => product,
            (false, true) => vendor,
            (false, false) => format!("{vendor} {product}"),
        };

        (model, serial, removable, bus_type)
    }

    fn bus_type_str(bus_type: u32) -> &'static str {
        match bus_type {
            BUS_TYPE_NVME => "NVMe",
            BUS_TYPE_SATA => "SATA SSD",
            BUS_TYPE_ATA => "HDD",
            BUS_TYPE_USB => "USB",
            BUS_TYPE_SCSI => "SCSI",
            BUS_TYPE_ATAPI => "ATAPI",
            _ => "Unknown",
        }
    }

    /// Returns the set of physical drive indices that contain the system volume.
    fn system_drive_indices() -> Vec<u32> {
        let sys_letter = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_owned());
        let vol_path = format!("\\\\.\\{}", sys_letter.trim_end_matches('\\'));
        let Some(hv) = WinHandle::open_ioctl(&vol_path) else {
            return vec![0]; // fallback: assume drive 0 is system
        };
        let mut extents = unsafe { std::mem::zeroed::<VolumeDiskExtents>() };
        let ok = ioctl_in_out(
            hv.0, IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS,
            std::ptr::null::<std::ffi::c_void>(), 0,
            &mut extents as *mut _ as *mut _,
            std::mem::size_of::<VolumeDiskExtents>() as u32,
        );
        if !ok { return vec![0]; }
        (0..extents.number_of_disk_extents.min(8) as usize)
            .map(|i| extents.extents[i].disk_number)
            .collect()
    }

    // ── Public list / wipe ───────────────────────────────────────────────────

    pub fn list_drives() -> anyhow::Result<Vec<PhysicalDriveInfo>> {
        let system_indices = system_drive_indices();
        let mut drives = Vec::new();

        for idx in 0u32..16 {
            let path = format!("\\\\.\\PhysicalDrive{idx}");
            let Some(h) = WinHandle::open_ioctl(&path) else {
                continue;
            };
            let Some((size_bytes, sector_size)) = geometry_ex(h.0) else {
                continue;
            };
            let (model, serial, is_removable, bus_type) = device_descriptor(h.0);
            let is_system = system_indices.contains(&idx);

            drives.push(PhysicalDriveInfo {
                path,
                model,
                serial,
                size_bytes,
                sector_size,
                media_type: bus_type_str(bus_type).to_owned(),
                is_removable,
                is_system,
            });
        }

        Ok(drives)
    }

    struct AlignedBuf {
        ptr: *mut u8,
        layout: Layout,
    }
    impl AlignedBuf {
        fn zeroed(size: usize, align: usize) -> Self {
            let layout = Layout::from_size_align(size, align).expect("bad layout");
            let ptr = unsafe { alloc_zeroed(layout) };
            assert!(!ptr.is_null(), "aligned alloc failed");
            AlignedBuf { ptr, layout }
        }
        fn as_slice(&self) -> &[u8] {
            unsafe { std::slice::from_raw_parts(self.ptr, self.layout.size()) }
        }
    }
    impl Drop for AlignedBuf {
        fn drop(&mut self) {
            unsafe { dealloc(self.ptr, self.layout) }
        }
    }

    pub fn raw_wipe(
        path: &str,
        cancel: Arc<AtomicBool>,
        progress: impl Fn(u64, u64, f64) + Send + 'static,
    ) -> anyhow::Result<u64> {
        // Open for write with no-buffering (required for aligned sector writes)
        let Some(guard) = WinHandle::open(
            path,
            GENERIC_READ | GENERIC_WRITE,
            FILE_FLAG_NO_BUFFERING | FILE_FLAG_WRITE_THROUGH,
        ) else {
            let err = unsafe { GetLastError() };
            if err == 5 {
                anyhow::bail!(
                    "Access denied opening {:?}. Run NitroShred as Administrator.",
                    path
                );
            }
            anyhow::bail!("Cannot open {:?} (Windows error {})", path, err);
        };

        let (total_bytes, sector_size) =
            geometry_ex(guard.0).context("Cannot read drive geometry")?;

        let buf_size = BUF_SIZE.max(sector_size as usize);
        let buf = AlignedBuf::zeroed(buf_size, BUF_ALIGN.max(sector_size as usize));

        let mut written_total = 0u64;
        let t0 = Instant::now();
        let mut last_report = Instant::now();

        while written_total < total_bytes {
            if cancel.load(Ordering::Relaxed) {
                anyhow::bail!("Cancelled by user");
            }

            let remaining = total_bytes - written_total;
            // Chunk must be a multiple of sector_size
            let raw_chunk = remaining.min(buf_size as u64);
            let chunk = ((raw_chunk + sector_size as u64 - 1) / sector_size as u64)
                * sector_size as u64;
            let chunk = chunk.min(buf_size as u64) as u32;

            let mut bytes_written = 0u32;
            let ok = unsafe {
                WriteFile(
                    guard.0,
                    buf.as_slice().as_ptr(),
                    chunk,
                    &mut bytes_written,
                    std::ptr::null::<std::ffi::c_void>(),
                )
            };
            if ok == 0 {
                let err = unsafe { GetLastError() };
                // ERROR_DISK_FULL(112) or ERROR_HANDLE_EOF(38) means we've hit the end — treat as done
                if err == 112 || err == 38 {
                    written_total += bytes_written as u64;
                    break;
                }
                anyhow::bail!("Write failed at offset {} (error {})", written_total, err);
            }

            written_total += bytes_written as u64;

            if last_report.elapsed().as_millis() >= 250 {
                let elapsed = t0.elapsed().as_secs_f64();
                let speed = if elapsed > 0.0 {
                    written_total as f64 / 1_048_576.0 / elapsed
                } else {
                    0.0
                };
                progress(written_total, total_bytes, speed);
                last_report = Instant::now();
            }
        }

        // Final progress report
        let elapsed = t0.elapsed().as_secs_f64();
        let speed = if elapsed > 0.0 {
            written_total as f64 / 1_048_576.0 / elapsed
        } else {
            0.0
        };
        progress(written_total, total_bytes, speed);

        Ok(written_total)
    }
}

// ─── Linux ───────────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
mod platform {
    use super::PhysicalDriveInfo;
    use std::fs;
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    use std::time::Instant;

    const BUF_SIZE: usize = 4 * 1024 * 1024;

    pub fn list_drives() -> anyhow::Result<Vec<PhysicalDriveInfo>> {
        let mut drives = Vec::new();

        // NVMe namespaces
        if let Ok(entries) = fs::read_dir("/sys/class/nvme") {
            for e in entries.flatten() {
                let ctrl = e.file_name().to_string_lossy().to_string();
                // Each controller has namespace devices: nvme0n1, nvme0n2, …
                if let Ok(ns_entries) = fs::read_dir(e.path()) {
                    for ns in ns_entries.flatten() {
                        let ns_name = ns.file_name().to_string_lossy().to_string();
                        if !ns_name.starts_with(&ctrl) {
                            continue;
                        }
                        let dev_path = format!("/dev/{ns_name}");
                        if let Some(info) = probe_linux_dev(&dev_path, false) {
                            drives.push(info);
                        }
                    }
                }
            }
        }

        // SCSI / SATA block devices
        if let Ok(entries) = fs::read_dir("/sys/class/block") {
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                // Only top-level disks (sda, sdb, vda…) — no partitions
                if !name.starts_with("sd") && !name.starts_with("vd") && !name.starts_with("hd") {
                    continue;
                }
                if name.chars().last().map(|c| c.is_ascii_digit()).unwrap_or(true) {
                    continue; // skip partitions like sda1
                }
                let dev_path = format!("/dev/{name}");
                if let Some(info) = probe_linux_dev(&dev_path, false) {
                    drives.push(info);
                }
            }
        }

        Ok(drives)
    }

    fn probe_linux_dev(path: &str, _removable: bool) -> Option<PhysicalDriveInfo> {
        let dev_name = std::path::Path::new(path).file_name()?.to_str()?.to_owned();

        // Size via /sys/class/block/<name>/size (in 512-byte sectors)
        let sys_size_path = format!("/sys/class/block/{dev_name}/size");
        let size_sectors: u64 = fs::read_to_string(&sys_size_path)
            .ok()?
            .trim()
            .parse()
            .ok()?;
        let size_bytes = size_sectors * 512;
        if size_bytes == 0 {
            return None;
        }

        // Sector size
        let sector_size: u32 = fs::read_to_string(format!(
            "/sys/class/block/{dev_name}/queue/logical_block_size"
        ))
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(512);

        // Model
        let model = fs::read_to_string(format!("/sys/class/block/{dev_name}/device/model"))
            .or_else(|_| {
                fs::read_to_string(format!("/sys/class/block/{dev_name}/../model"))
            })
            .map(|s| s.trim().to_owned())
            .unwrap_or_else(|_| "Unknown".to_owned());

        // Serial (optional)
        let serial = fs::read_to_string(format!("/sys/class/block/{dev_name}/device/serial"))
            .map(|s| s.trim().to_owned())
            .unwrap_or_default();

        // Removable
        let is_removable = fs::read_to_string(format!("/sys/class/block/{dev_name}/removable"))
            .map(|s| s.trim() == "1")
            .unwrap_or(false);

        // Media type
        let media_type = if dev_name.starts_with("nvme") {
            "NVMe".to_owned()
        } else {
            // rotational = 1 → HDD, 0 → SSD
            let rotational = fs::read_to_string(format!(
                "/sys/class/block/{dev_name}/queue/rotational"
            ))
            .map(|s| s.trim() == "1")
            .unwrap_or(true);
            if rotational {
                "HDD".to_owned()
            } else {
                "SATA SSD".to_owned()
            }
        };

        // System drive = contains /
        let is_system = is_system_drive(&dev_name);

        Some(PhysicalDriveInfo {
            path: path.to_owned(),
            model,
            serial,
            size_bytes,
            sector_size,
            media_type,
            is_removable,
            is_system,
        })
    }

    fn is_system_drive(dev_name: &str) -> bool {
        let Ok(mounts) = fs::read_to_string("/proc/mounts") else {
            return false;
        };
        for line in mounts.lines() {
            let mut parts = line.split_whitespace();
            let device = parts.next().unwrap_or("");
            let mountpoint = parts.next().unwrap_or("");
            if mountpoint == "/" && device.contains(dev_name) {
                return true;
            }
        }
        false
    }

    pub fn raw_wipe(
        path: &str,
        cancel: Arc<AtomicBool>,
        progress: impl Fn(u64, u64, f64) + Send + 'static,
    ) -> anyhow::Result<u64> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .custom_flags(libc::O_DIRECT | libc::O_SYNC)
            .open(path)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    anyhow::anyhow!("Permission denied opening {:?}. Run as root.", path)
                } else {
                    anyhow::anyhow!("Cannot open {:?}: {}", path, e)
                }
            })?;

        // Get size via ioctl BLKGETSIZE64
        let size_bytes: u64 = unsafe {
            let mut sz: u64 = 0;
            let fd = std::os::unix::io::AsRawFd::as_raw_fd(&file);
            let ret = libc::ioctl(fd, 0x80081272u64, &mut sz); // BLKGETSIZE64
            if ret != 0 {
                anyhow::bail!("Cannot get drive size via BLKGETSIZE64");
            }
            sz
        };

        // Aligned zeroed buffer
        let layout = std::alloc::Layout::from_size_align(BUF_SIZE, 4096).unwrap();
        let ptr = unsafe { std::alloc::alloc_zeroed(layout) };
        let buf = unsafe { std::slice::from_raw_parts(ptr, BUF_SIZE) };

        let mut written_total = 0u64;
        let t0 = Instant::now();
        let mut last_report = Instant::now();

        while written_total < size_bytes {
            if cancel.load(Ordering::Relaxed) {
                unsafe { std::alloc::dealloc(ptr, layout) };
                anyhow::bail!("Cancelled by user");
            }

            let remaining = size_bytes - written_total;
            let chunk = remaining.min(BUF_SIZE as u64) as usize;

            match file.write_all(&buf[..chunk]) {
                Ok(_) => {}
                Err(e) if e.raw_os_error() == Some(libc::ENOSPC) => break,
                Err(e) => {
                    unsafe { std::alloc::dealloc(ptr, layout) };
                    anyhow::bail!("Write error at offset {}: {}", written_total, e);
                }
            }

            written_total += chunk as u64;

            if last_report.elapsed().as_millis() >= 250 {
                let elapsed = t0.elapsed().as_secs_f64();
                let speed = written_total as f64 / 1_048_576.0 / elapsed.max(0.001);
                progress(written_total, size_bytes, speed);
                last_report = Instant::now();
            }
        }

        unsafe { std::alloc::dealloc(ptr, layout) };

        let elapsed = t0.elapsed().as_secs_f64();
        let speed = written_total as f64 / 1_048_576.0 / elapsed.max(0.001);
        progress(written_total, size_bytes, speed);

        Ok(written_total)
    }
}

// ─── Other platforms (stub) ──────────────────────────────────────────────────

#[cfg(not(any(windows, target_os = "linux")))]
mod platform {
    use super::PhysicalDriveInfo;
    use std::sync::{atomic::AtomicBool, Arc};

    pub fn list_drives() -> anyhow::Result<Vec<PhysicalDriveInfo>> {
        Ok(Vec::new())
    }

    pub fn raw_wipe(
        _path: &str,
        _cancel: Arc<AtomicBool>,
        _progress: impl Fn(u64, u64, f64),
    ) -> anyhow::Result<u64> {
        anyhow::bail!("Physical drive wipe is not supported on this platform");
    }
}
