use std::path::Path;

#[cfg(target_os = "linux")]
pub fn try_trim(path: &Path) -> std::io::Result<bool> {
    use std::fs::OpenOptions;
    use std::os::unix::io::AsRawFd;

    let file = OpenOptions::new().write(true).open(path)?;
    let size = file.metadata()?.len();

    if size == 0 {
        return Ok(true);
    }

    // fallocate PUNCH_HOLE instructs the filesystem to release the physical blocks
    // backing this file range. On SSDs, the filesystem passes TRIM commands to the
    // drive controller for exactly those sectors — no manual extent mapping needed.
    let ret = unsafe {
        libc::fallocate(
            file.as_raw_fd(),
            libc::FALLOC_FL_PUNCH_HOLE | libc::FALLOC_FL_KEEP_SIZE,
            0,
            size as libc::off_t,
        )
    };

    if ret == 0 {
        file.sync_all()?;
        Ok(true)
    } else {
        match std::io::Error::last_os_error().raw_os_error() {
            Some(libc::EOPNOTSUPP) | Some(libc::ENOSYS) | Some(libc::EINVAL) => Ok(false),
            _ => Err(std::io::Error::last_os_error()),
        }
    }
}

#[cfg(windows)]
pub fn try_trim(path: &Path) -> std::io::Result<bool> {
    use std::fs::OpenOptions;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::System::IO::DeviceIoControl;

    const FSCTL_FILE_LEVEL_TRIM: u32 = 0x0009_00BC;

    #[repr(C)]
    struct FileAllocatedRangeBuffer {
        file_offset: i64,
        length: i64,
    }

    let file = OpenOptions::new().write(true).open(path)?;
    let size = file.metadata()?.len();

    if size == 0 {
        return Ok(true);
    }

    let range = FileAllocatedRangeBuffer {
        file_offset: 0,
        length: size as i64,
    };

    let mut bytes_returned = 0u32;
    let ok = unsafe {
        DeviceIoControl(
            file.as_raw_handle() as HANDLE,
            FSCTL_FILE_LEVEL_TRIM,
            &range as *const _ as *mut _,
            std::mem::size_of::<FileAllocatedRangeBuffer>() as u32,
            std::ptr::null_mut(),
            0,
            &mut bytes_returned,
            std::ptr::null_mut(),
        )
    };

    if ok != 0 {
        file.sync_all()?;
        Ok(true)
    } else {
        // Unsupported on HDD or older FS — fall through to zero-fill
        Ok(false)
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
pub fn try_trim(_path: &Path) -> std::io::Result<bool> {
    Ok(false)
}
