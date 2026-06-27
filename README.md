# NitroShred

NitroShred permanently deletes files so they can never be recovered — not by software, not by forensic tools. It's built to be as fast as your storage hardware allows.

---

## Why not just delete normally?

When you delete a file the normal way, the data is still physically on your drive. The operating system just marks that space as "available" — the actual content sits there until something else overwrites it. Recovery tools can bring it back in seconds.

NitroShred overwrites the file's contents before deleting it, then wipes its name and history. There's nothing left to recover.

### What about SSDs?

SSDs have an extra complication. Even if you overwrite a file, the drive's internal controller often writes the new data to a fresh location and keeps the old data sitting in another spot. NitroShred handles this by sending a direct erase command to the drive hardware itself, so the original data is physically gone.

---

## Install

```bash
cargo build --release
```

The built program will be at `target/release/nitroshred`.

---

## Usage

```
nitroshred [OPTIONS] <PATH>
```

| Option | What it does |
|---|---|
| `<PATH>` | The file or folder you want to permanently delete |
| `-f, --force` | Delete even if the file is marked as read-only |
| `-r, --recursive` | Delete an entire folder and everything inside it |
| `-v, --verbose` | Show progress and speed while running |
| `--no-trim` | Don't use the SSD fast-erase method, use overwriting instead |

**Examples**

```bash
# Permanently delete a single file
nitroshred secret.pdf

# Permanently delete an entire folder
nitroshred -r /home/user/private-documents

# See what's happening while it runs
nitroshred -r -v /home/user/private-documents
```

---

## Safety

NitroShred will refuse to run if you point it at a system folder — even if you use `--force`. This protects your operating system from being accidentally wiped.

Protected locations include: `/`, `/etc`, `/bin`, `/usr`, `C:\Windows`, and similar.

---

## Platforms

| Platform | Status |
|---|---|
| Linux | Fully supported |
| Windows | Supported (SSD fast-erase coming soon) |
