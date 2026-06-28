#![cfg(target_os = "linux")]

use io_uring::{opcode, types, IoUring};
use std::alloc::{alloc_zeroed, dealloc, Layout};
use std::fs::OpenOptions;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use std::path::Path;

const BUF_SIZE: usize = 8 * 1024 * 1024;
const BUF_ALIGN: usize = 4096;
const RING_DEPTH: u32 = 128;

struct AlignedBuf {
    ptr: *mut u8,
    layout: Layout,
}

impl AlignedBuf {
    fn zeroed() -> Self {
        let layout = Layout::from_size_align(BUF_SIZE, BUF_ALIGN).unwrap();
        let ptr = unsafe { alloc_zeroed(layout) };
        assert!(!ptr.is_null(), "aligned allocation failed");
        AlignedBuf { ptr, layout }
    }
}

impl Drop for AlignedBuf {
    fn drop(&mut self) {
        unsafe { dealloc(self.ptr, self.layout) }
    }
}

pub fn shred_uring(path: &Path) -> std::io::Result<()> {
    let file = OpenOptions::new()
        .write(true)
        .custom_flags(libc::O_DIRECT)
        .open(path)?;

    let total = file.metadata()?.len();
    let mut ring = IoUring::new(RING_DEPTH)?;
    let buffer = AlignedBuf::zeroed();
    let mut offset = 0u64;

    while offset < total {
        let chunk = ((total - offset) as usize).min(BUF_SIZE) as u32;

        let write_e = opcode::Write::new(
            types::Fd(file.as_raw_fd()),
            buffer.ptr,
            chunk,
        )
        .offset(offset)
        .build();

        unsafe {
            ring.submission().push(&write_e).map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::Other, "io_uring submission queue full")
            })?;
        }
        ring.submit_and_wait(1)?;

        // Check CQE for errors
        if let Some(cqe) = ring.completion().next() {
            if cqe.result() < 0 {
                return Err(std::io::Error::from_raw_os_error(-cqe.result()));
            }
        }

        offset += chunk as u64;
    }

    file.sync_all()?;
    Ok(())
}
