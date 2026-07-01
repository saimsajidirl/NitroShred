# NitroShred

**Permanent secure data erasure — fast, open source, and free forever.**

NitroShred permanently deletes your private files so they can never be brought back — not with recovery software, not by a technician, and not by law enforcement forensic labs. When you shred with NitroShred, data is overwritten and destroyed at the source.

---

## Why isn't the Recycle Bin enough?

When you delete a file normally, your computer doesn't actually erase it. It marks that space as "available" and hides the file. The actual content stays on your disk until something else overwrites it.

That's why "undelete" and recovery tools exist — they can often restore deleted files in seconds. Emptying the Recycle Bin does not truly destroy the data.

### What about SSDs?

SSDs have an extra complication. Even if you overwrite a file, the drive's controller may write new data to a fresh location and keep old data in another spot. NitroShred handles this by sending **TRIM** commands to the drive hardware when possible, so blocks are physically released.

---

## What does NitroShred do?

1. **Overwrites your files** — Every byte is replaced with zeros.
2. **Removes the file completely** — File name and trace are wiped.
3. **Wipes all free space** (Drive mode) — Fills every remaining free cluster with zeros so old deleted data can't be recovered.
4. **Hardware TRIM on SSDs** — Sends TRIM commands so drive firmware permanently erases blocks.
5. **Raw sector wipe** (Physical mode) — Bypasses the file system entirely and zeroes every single physical sector. The drive is indistinguishable from one fresh out of the factory.

---

## Modes

### Folder
Permanently destroy everything inside one folder — documents, photos, backups, private files. The rest of the drive is untouched.

### Drive
Full secure wipe for an entire USB stick or external drive: shred all files, overwrite every free cluster, then TRIM the drive. Use before selling, donating, or disposing of storage.

### Physical
The deepest level of erasure available. Bypasses the file system and writes zeros directly to every physical sector on the disk via raw device I/O (DBAN / nwipe style). Also supports **NVMe Sanitize** (hardware crypto erase or block erase) and **bootable offline wipe script** export for environments where the drive cannot be unmounted.

#### Drive vs Physical — the key distinction

**Drive mode** destroys what the data said, but its ghost remains. A forensic expert might still be able to tell that *something* once existed in that space — they just cannot read what it was. Like a redacted document: the black bars are there. You know something was written. You just cannot read it.

**Physical mode** erases the ghost too. Every sector is overwritten with zeros until the entire drive is indistinguishable from one that has never been touched. There is no trace, no record, no proof that anything was ever there. It is as if the data never existed at all — not hidden, not redacted, simply *gone from existence*.

For most people, Drive mode is more than enough. Physical mode is for when you need the drive to look like it just came out of the box — no history, no past, nothing.

### Physical sub-methods

| Method | Description |
|--------|-------------|
| **Raw Sector Wipe** | Writes zeros to every sector — nwipe / DBAN style. Works on any drive type. Requires admin / root. |
| **NVMe Sanitize — Crypto Erase** | Drive firmware destroys its own encryption keys, making all data instantly unreadable. Fastest and most thorough for NVMe SSDs. |
| **NVMe Sanitize — Block Erase** | Firmware erases every NAND block via the internal sanitize command. For NVMe drives without per-sector encryption. |
| **Export Bootable Wipe Script** | Saves a nwipe-compatible bash script + instructions for offline wiping from a live Linux USB (ShredOS, Ubuntu Live, etc.). |

---

## When should I use it?

- Before selling or giving away a computer, laptop, or external drive
- When getting rid of sensitive personal documents or private photos
- When you need files gone for good — not just hidden
- When handing a drive to someone you do not fully trust
- When preparing a device for decommission or disposal

---

## Fun fact (spoiler alert)

In the true-crime documentary *Lover, Stalker, Killer*, a killer deleted evidence from an SD card and assumed it was gone. They only hit "delete" — they didn't securely wipe the data. An IT specialist recovered the "deleted" files and found critical evidence. The killer was caught.

If they had used NitroShred, that data would have been truly unrecoverable. Normal delete is not secure erase.

---

## Why is NitroShred free?

NitroShred is **open source** — the full code is public and anyone can read exactly how it works. No hidden tricks, no spyware, no locked-in secrets.

NitroShred is free and **will always be free**. We will never compromise your data or lock features behind a paywall. You can inspect the code yourself and verify that it does what it says.

### Support the creator

Love NitroShred? The app stays free forever — but if you want to help the creator keep building:

- [Sponsor on GitHub](https://github.com/sponsors/saimsajidirl)
- [Patreon & links on GitHub](https://github.com/saimsajidirl/NitroShred)
- Email: [saimsajidirl@gmail.com](mailto:saimsajidirl@gmail.com)

---

## Safety

- **This cannot be undone.** Once shredded, your files are gone forever.
- **Your main system drive is protected.** NitroShred will not wipe Windows or your primary `C:` drive, and physical wipe is blocked on any drive the OS is running from.
- **Double-check before you click.** Make sure you've selected the right folder or drive.
- **Physical mode requires admin privileges.** Run NitroShred as Administrator (Windows) or root (Linux) for raw sector wipe and NVMe sanitize operations.

Protected paths include `/`, `/etc`, `/bin`, `/usr`, `C:\Windows`, and similar system locations.

---

## Install

### Prerequisites

- [Rust](https://rustup.rs/) (latest stable)
- **Windows:** MSVC build tools, WebView2
- **Desktop app:** [Tauri CLI](https://tauri.app/) — `cargo install tauri-cli --locked`

### CLI

```bash
cargo build --release
```

Binary: `target/release/nitroshred` (or `nitroshred.exe` on Windows)

### Desktop app

```bash
cd app/src-tauri
cargo tauri dev      # development
cargo tauri build    # release installer
```

---

## Usage

### CLI

```
nitroshred [OPTIONS] <PATH>
```

| Option | Description |
|--------|-------------|
| `<PATH>` | File or directory to shred |
| `-r, --recursive` | Shred an entire folder tree |
| `-v, --verbose` | Show per-file speed and I/O method |
| `--full` | **Full drive wipe:** shred files + overwrite free space + volume TRIM |

**Examples**

```bash
# Shred a single file
nitroshred secret.pdf

# Shred everything inside a folder
nitroshred -r ./private-documents

# Full secure wipe of an external drive (D: on Windows)
nitroshred -r --full D:\

# Full wipe of a mounted USB on Linux
nitroshred -r --full /media/user/USB

# Verbose output
nitroshred -r -v --full D:\
```

### Desktop app

1. Open **Shred** tab → choose **Folder**, **Drive**, or **Physical**
2. Select target → configure options → confirm
3. **Drive mode** runs the full 3-phase wipe automatically (files → free space → TRIM)
4. **Physical mode** lists attached physical drives — select a drive and choose your erase method

---

## Platforms

| Platform | Status |
|----------|--------|
| Windows | Fully supported — file TRIM, free-space wipe, volume retrim, raw sector wipe, NVMe sanitize |
| Linux | Fully supported — TRIM, io_uring, fstrim, raw sector wipe, NVMe ioctl |
| macOS | CLI core works; desktop app not yet tested |

---

## Project structure

```
NitroShred/
├── core/                        # Shred engine (shared by CLI + app)
│   └── src/
│       ├── lib.rs               # Public API + shred orchestration
│       ├── select_erase_method  # Per-file erase strategy picker
│       ├── write_zeros_direct   # Direct I/O zero-fill (Windows & Linux)
│       ├── write_zeros_parallel # Parallel file shredder (rayon)
│       ├── write_zeros_uring    # io_uring zero-fill (Linux only)
│       ├── wipe_free_space      # Free cluster overwrite
│       ├── volume_trim          # Volume-level TRIM (fstrim / IOCTL)
│       ├── trim_ssd_blocks      # Per-file TRIM
│       ├── block_protected_paths# System path guard
│       ├── physical_drive_wipe  # Raw sector wipe + drive enumeration
│       ├── hardware_secure_erase# NVMe Sanitize / ATA Secure Erase
│       └── bootable_script      # Bootable offline wipe script generator
├── src/                         # CLI binary
├── app/
│   ├── ui/                      # Desktop app frontend (HTML/CSS/JS)
│   └── src-tauri/               # Tauri backend
│       ├── src/
│       │   ├── lib.rs           # Tauri command registration
│       │   └── shred_commands   # All Tauri command handlers
│       ├── capabilities/        # Tauri permissions (commit this)
│       └── permissions/         # Custom command ACLs (commit this)
└── scripts/                     # Icon generation helper
```

> **Note:** `app/src-tauri/gen/` is auto-generated by Tauri at build time — do **not** commit it. Run `cargo tauri dev` or `cargo tauri build` to regenerate.

---

## License

Open source. See repository for license details.

**NitroShred v2.1** · Permanent secure erasure
