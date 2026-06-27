# NitroShred

High-performance bare-metal data invalidation engine. Targets physical line-rate saturation of storage media (≥ 3,500 MB/s on NVMe PCIe Gen4) through hardware-aligned overwrites, OS cache bypassing, and async I/O.

---

## How it works

NitroShred picks the fastest safe erasure path per file at runtime:

1. **Hardware TRIM** (SSD/NVMe primary) — `fallocate(PUNCH_HOLE)` instructs the filesystem to release physical flash blocks directly. The kernel passes TRIM commands to the drive controller for exactly those sectors. Near-instant on SSDs.
2. **Parallel pwrite** (files ≥ 512 MB) — splits the file into N segments and writes zeros concurrently via `pwrite()` at independent byte offsets, saturating NVMe's internal command queue depth.
3. **io_uring** (Linux, HDD / TRIM-unsupported) — zero-context-switch async ring buffer. Batches write submissions without per-operation syscalls.
4. **O_DIRECT zero-fill** (fallback / Windows) — bypasses the OS page cache, feeding data straight to the device controller DMA channel.

After the I/O path completes, every file goes through a metadata pipeline: truncate → fsync → rename to a random name → unlink.

---

## Optimizations

| # | Optimization | Gain |
|---|---|---|
| 01 | O_DIRECT — bypasses OS page cache | baseline |
| 02 | 8 MB aligned buffers — reduces syscall count 128× vs 64 KB | +10–20% |
| 03 | Single-pass zero overwrite | — |
| 04 | Rayon thread pool — parallel multi-file processing | scales with core count |
| 05 | io_uring async ring buffer | 2–5× on NVMe |
| 06 | Hardware TRIM / fallocate PUNCH_HOLE | ~100× vs zero-fill on SSD |
| 07 | Intra-file parallel pwrite (files > 512 MB) | 1.5–2× |
| 08 | Async metadata pipeline — overlapped rename/unlink | +5–15% multi-file |

---

## Install

```bash
cargo build --release
# binary at target/release/nitroshred
```

Requires Rust 1.75+. Linux kernel 5.1+ for io_uring support.

---

## Usage

```
nitroshred [OPTIONS] <PATH>
```

| Flag | Description |
|---|---|
| `<PATH>` | File or directory to shred |
| `-f, --force` | Bypass read-only permissions |
| `-r, --recursive` | Recursively shred a directory tree |
| `-v, --verbose` | Print I/O path and speed per file |
| `--no-trim` | Disable hardware TRIM, force zero-fill on SSD targets |

**Examples**

```bash
# Shred a single file
nitroshred secret.db

# Shred a directory tree verbosely
nitroshred -r -v /mnt/data/vault

# Force zero-fill path even on NVMe
nitroshred --no-trim large_file.bin
```

---

## Safety

Execution is blocked unconditionally — before any I/O, regardless of `--force` — if the target path is or is inside a protected system directory:

```
/  /bin  /boot  /dev  /etc  /lib  /lib64  /proc  /run  /sbin  /sys  /usr
C:\  C:\Windows  C:\Program Files
```

---

## SSD / NVMe note

Traditional software zero-fill does not reliably erase data on SSDs. The Flash Translation Layer (FTL) uses wear-leveling to remap writes to freshly allocated blocks, leaving original physical cells intact. NitroShred's TRIM path bypasses this by instructing the drive controller to erase the original flash blocks directly.

Use `--no-trim` only on HDD targets or when benchmarking the zero-fill path.

---

## Platform

| Platform | TRIM | io_uring | O_DIRECT |
|---|---|---|---|
| Linux | `fallocate(PUNCH_HOLE)` | ✓ | ✓ |
| Windows | — (zero-fill fallback) | — | `FILE_FLAG_NO_BUFFERING` (planned) |
