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
3. **Wipes all free space** (drive mode) — Fills every remaining free cluster with zeros so old deleted data can't be recovered.
4. **Hardware TRIM on SSDs** — Sends TRIM commands so drive firmware permanently erases blocks.

### Folder vs drive

| Mode | What it does |
|------|----------------|
| **Shred a folder** | Permanently destroys everything inside one folder — documents, photos, backups, etc. |
| **Wipe a drive** | Full secure wipe for a USB stick or external drive: shred all files, overwrite every free cluster, then TRIM the drive. Use before selling, donating, or disposing of storage. |

### When should I use it?

- Before selling or giving away a computer or external drive
- When getting rid of sensitive personal documents
- When you need files gone for good — not just hidden

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
- **Your main system drive is protected.** NitroShred will not wipe Windows or your primary `C:` drive.
- **Double-check before you click.** Make sure you've selected the right folder or drive.

Protected paths include `/`, `/etc`, `/bin`, `/usr`, `C:\Windows`, and similar.

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

1. Open **Shred** tab → choose **Folder** or **Drive**
2. Select target → configure options → confirm
3. **Drive mode** runs the full 3-phase wipe automatically

---

## Platforms

| Platform | Status |
|----------|--------|
| Linux | Fully supported — TRIM, io_uring, fstrim |
| Windows | Supported — file TRIM, free-space wipe, volume retrim |
| macOS | CLI core works; desktop app not yet tested |

---

## Project structure

```
NitroShred/
├── core/                 # Shred engine (shared by CLI + app)
├── src/                  # CLI binary
├── app/
│   ├── ui/               # Desktop app frontend
│   └── src-tauri/        # Tauri backend
│       ├── capabilities/ # Tauri permissions (commit this)
│       └── permissions/  # Custom command ACLs (commit this)
└── scripts/              # Icon generation helper
```

> **Note:** `app/src-tauri/gen/` is auto-generated by Tauri at build time — do **not** commit it. Run `cargo tauri dev` or `cargo tauri build` to regenerate.

---

## License

Open source. See repository for license details.

**NitroShred v2.0** · Permanent secure erasure
