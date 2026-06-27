#![cfg(target_os = "linux")]

use io_uring::{opcode, types, IoUring};
use std::fs::OpenOptions;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use std::path::Path;

const BUF_SIZE: usize = 8 * 1024 * 1024;
const RING_DEPTH: u32 = 128;

pub fn shred_uring(path: &Path) -> std::io::Result<()> {
    let file = OpenOptions::new()
        .write(true)
        .custom_flags(libc::O_DIRECT)
        .open(path)?;

    let total = file.metadata()?.len();
    let mut ring = IoUring::new(RING_DEPTH)?;
    // Buffer must be kept alive for the duration of the ring submission
    let buffer = vec![0u8; BUF_SIZE];
    let mut offset = 0u64;

    while offset < total {
        let chunk = ((total - offset) as usize).min(BUF_SIZE) as u32;

        let write_e = opcode::Write::new(
            types::Fd(file.as_raw_fd()),
            buffer.as_ptr(),
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
