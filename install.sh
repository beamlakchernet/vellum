#!/bin/bash
set -e

echo "🎵 Vellum Installation Script"
echo "============================="
echo ""

# Check if cargo is installed
if ! command -v cargo &> /dev/null; then
    echo "❌ Cargo not found. Please install Rust from https://rustup.rs/"
    exit 1
fi

echo "✓ Cargo found"

# Install vellum binary
echo ""
echo "📦 Installing vellum binary..."
cargo install --git https://github.com/beamlakchernet/vellum

# Check if syncedlyrics is already installed
if command -v syncedlyrics &> /dev/null; then
    echo "✓ syncedlyrics already installed"
else
    # Check if pip is available
    if ! command -v pip3 &> /dev/null && ! command -v pip &> /dev/null; then
        echo "❌ Python pip not found. Please install Python 3 and pip."
        echo "   Then run: pip install syncedlyrics"
        exit 1
    fi

    # Determine which pip command to use
    PIP_CMD=$(command -v pip3 || command -v pip)

    echo ""
    echo "📥 Installing syncedlyrics Python package..."
    "$PIP_CMD" install syncedlyrics

    if ! command -v syncedlyrics &> /dev/null; then
        echo "⚠️  syncedlyrics installed but not found on PATH."
        echo "   Try running 'hash -r' to refresh your shell's command cache."
        echo "   Or restart your terminal."
    fi
fi

echo ""
echo "✅ Installation complete!"
echo ""
echo "Usage:"
echo "  vellum --from-player     # Follow current MPRIS player"
echo "  vellum --file song.lrc   # Play from local file"
echo ""
echo "For more info, run: vellum --help"
