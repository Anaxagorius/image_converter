# Image Converter

A fast, user-friendly batch image format converter built in Rust with **egui**.

Drag & drop multiple images → pick a target format → convert.

![screenshot placeholder](assets/screenshot.png)

## Supported formats

| Format | Notes |
|--------|-------|
| PNG    | Lossless, supports transparency |
| JPEG   | Lossy, great for photos, no alpha |
| WebP   | Modern format, excellent compression |
| TIFF   | High quality, wide compatibility |
| BMP    | Uncompressed bitmap |
| GIF    | 256-colour limit |

> **HEIC note**: Full HEIC support requires the system `libheif` library (and has patent
> complications). This version sticks to pure-Rust codecs so it compiles cleanly on any
> machine with just `cargo`. HEIC can be added later if needed.

## Requirements

- [Rust](https://rustup.rs/) 1.80 or newer
- Windows, macOS, or Linux

## Build & Run

```bash
cargo run --release
```

The release binary is placed in `target/release/image-converter` (or `.exe` on Windows).

### Optional: install system-wide

```bash
cargo install --path .
```

Then just run `image-converter` from anywhere in your terminal.

## Features

- **Drag & drop** multiple files onto the window, or use **Add files…**
- **Duplicate detection** — the same file is never added twice
- **File size display** — see how large each source image is before converting
- **Choose output format** with an inline description of each format
- **Quality slider** for JPEG and WebP output
- **Save next to originals** or to a **custom output folder**
- **Parallel conversion** — uses all CPU cores via [rayon](https://github.com/rayon-rs/rayon)
- **Live per-file status** with colour-coded icons (⏳ pending · 🔄 converting · ✅ done · ❌ error)
- **Overall progress bar** with percentage during batch conversion
- **Open output folder** button appears automatically after conversion finishes
- **Retry failed** — clicking Convert again re-queues any files that errored
- **Remove done / Remove errors** quick-filter buttons to clean up the list
- **Enter key** shortcut to start conversion
- Full error details shown on hover for any file that failed
- Non-blocking UI — the window stays responsive throughout

## Notes

- When converting to the same format as the source, the tool appends `_converted` to the
  filename to avoid overwriting the original.
- WebP output is currently **lossless** (the `image` crate does not yet expose a stable
  lossy-WebP encoder). Lossless WebP is already significantly smaller than PNG for most
  photographic content.
- Conversion runs fully in a background thread pool — you can keep working in the UI while
  files are being processed.
