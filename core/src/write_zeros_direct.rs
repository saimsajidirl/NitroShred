use std::alloc::{alloc_zeroed, dealloc, Layout};
use std::fs::{remove_file, rename, OpenOptions};
use std::io::Write;
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

const BUF_SIZE: usize = 8 * 1024 * 1024;
const BUF_ALIGN: usize = 4096; // O_DIRECT requires buffer aligned to device block size

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

    fn as_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr, self.layout.size()) }
    }
}

impl Drop for AlignedBuf {
    fn drop(&mut self) {
        unsafe { dealloc(self.ptr, self.layout) }
    }
}

pub fn zero_fill(path: &Path) -> std::io::Result<()> {
    let mut opts = OpenOptions::new();
    opts.write(true);

    #[cfg(unix)]
    opts.custom_flags(libc::O_DIRECT);

    let mut file = opts.open(path)?;
    let total = file.metadata()?.len();
    let buf = AlignedBuf::zeroed();
    let mut written = 0u64;

    while written < total {
        let chunk = ((total - written) as usize).min(BUF_SIZE);
        file.write_all(&buf.as_slice()[..chunk])?;
        written += chunk as u64;
    }

    file.sync_all()?;
    Ok(())
}

pub fn scramble_metadata(path: &Path) -> std::io::Result<()> {
    {
        let file = OpenOptions::new().write(true).open(path)?;
        file.set_len(0)?;
        file.sync_all()?; // flush truncation to disk before rename
    }

    let scrambled = path.with_file_name(format!("ns_{:08x}", rand::random::<u32>()));
    rename(path, &scrambled)?;
    remove_file(&scrambled)?;
    Ok(())
}
