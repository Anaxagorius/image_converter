#!/bin/bash
set -e

echo "============================================"
echo " Image Converter - Bootstrap Installer"
echo "============================================"
echo

# Check for git
if ! command -v git &>/dev/null; then
    echo "[ERROR] git is not installed."
    if [[ "$OSTYPE" == "darwin"* ]]; then
        echo "Install Xcode Command Line Tools by running: xcode-select --install"
    else
        echo "Install git via your package manager (e.g. sudo apt install git)"
    fi
    exit 1
fi

# Check for cargo / install Rust if missing
if ! command -v cargo &>/dev/null; then
    echo "Rust/cargo not found. Installing Rust via rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
    # Source cargo env for this session
    source "$HOME/.cargo/env"
    echo
    echo "Rust installed successfully."
fi

# Source cargo env in case it wasn't already on PATH
[ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env"

INSTALL_DIR="$HOME/image_converter"

if [ -d "$INSTALL_DIR" ]; then
    if [ ! -d "$INSTALL_DIR/.git" ]; then
        echo "[ERROR] $INSTALL_DIR exists but is not a git repository. Please remove it and re-run."
        exit 1
    fi
    echo "Repository already exists at $INSTALL_DIR. Pulling latest changes..."
    cd "$INSTALL_DIR"
    if ! git pull; then
        echo "[ERROR] git pull failed. Check for merge conflicts or network issues."
        exit 1
    fi
else
    echo "Cloning repository..."
    git clone https://github.com/Anaxagorius/image_converter.git "$INSTALL_DIR"
    cd "$INSTALL_DIR"
fi

echo
echo "Building image-converter (this may take several minutes on first run)..."
cargo build --release

echo
echo "============================================"
echo " Build complete!"
echo " Binary: $INSTALL_DIR/target/release/image-converter"
echo "============================================"
echo

read -p "Launch image-converter now? (y/N): " LAUNCH
if [[ "$LAUNCH" =~ ^[Yy]$ ]]; then
    "$INSTALL_DIR/target/release/image-converter" &
fi
