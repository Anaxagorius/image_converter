# Image Converter

Simple batch image format converter built in Rust with **egui**.

Drag & drop multiple images → pick a target format → convert.

## Supported formats

**Input / Output**
- PNG
- JPEG / JPG
- WebP
- GIF
- BMP
- TIFF

> **HEIC note**: Full HEIC support needs the system `libheif` library (and has patent complications).  
> This first version sticks to pure-Rust codecs so it compiles cleanly on any machine with just `cargo`.  
> You can add HEIC later if needed.

## Requirements

- [Rust](https://rustup.rs/) (1.80+ recommended)
- Windows / macOS / Linux

## Build & Run

```bash
cd image-converter
cargo run --release
```

The release binary will be in `target/release/image-converter` (or `.exe` on Windows).

### One-liner install (optional)

```bash
cargo install --path .
```

Then just run `image-converter` from anywhere.

## Features

- Drag & drop multiple files
- Or use the “Add files…” button
- Choose target format + quality (JPEG)
- Save next to originals **or** to a chosen folder
- Parallel conversion (uses all CPU cores via rayon)
- Live status per file

## Notes

- When converting to the same format as the source, the tool appends `_converted` to avoid overwriting.
- WebP is currently saved lossless (still much smaller than PNG for most photos). Lossy WebP can be added later if desired.
- The UI stays responsive while converting.
