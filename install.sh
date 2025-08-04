#!/bin/bash

set -e

echo "🚀 Installing Scriba..."

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ] || [ ! -f "src/main.rs" ]; then
    echo "❌ Error: Please run this script from the Scriba project directory."
    exit 1
fi

# Build the release version
echo "🔨 Building Scriba in release mode..."
cargo build --release

# Copy to a standard location
INSTALL_DIR="$HOME/.local/bin"
mkdir -p "$INSTALL_DIR"

echo "📦 Installing Scriba to $INSTALL_DIR..."
cp target/release/scriba "$INSTALL_DIR/"

# Make sure it's executable
chmod +x "$INSTALL_DIR/scriba"

# Check if ~/.local/bin is in PATH
if [[ ":$PATH:" != *":$HOME/.local/bin:"* ]]; then
    echo "⚠️  Warning: $HOME/.local/bin is not in your PATH."
    echo "   Add this line to your shell config file (~/.bashrc, ~/.zshrc, etc.):"
    echo "   export PATH=\"\$HOME/.local/bin:\$PATH\""
    echo ""
fi

# Download Vosk library to system location if not present
VOSK_LIB_DIR="/usr/local/lib"
if [ ! -f "$VOSK_LIB_DIR/libvosk.so" ] && [ ! -f "$VOSK_LIB_DIR/libvosk.dylib" ]; then
    echo "📥 Downloading Vosk library for system-wide installation..."
    
    # Create temporary directory
    TEMP_DIR=$(mktemp -d)
    cd "$TEMP_DIR"
    
    # Determine platform and download appropriate Vosk library
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        VOSK_FILE="vosk-linux-x86_64-0.3.45.zip"
        LIB_FILE="libvosk.so"
    elif [[ "$OSTYPE" == "darwin"* ]]; then
        VOSK_FILE="vosk-osx-0.3.45.zip"
        LIB_FILE="libvosk.dylib"
    else
        echo "Unsupported platform: $OSTYPE"
        exit 1
    fi
    
    # Download and extract Vosk library
    curl -L -o vosk-lib.zip "https://github.com/alphacep/vosk-api/releases/download/v0.3.45/$VOSK_FILE"
    unzip -q vosk-lib.zip
    
    # Copy library to system location (requires sudo)
    echo "🔐 Installing Vosk library system-wide (requires sudo)..."
    sudo cp vosk-*/"$LIB_FILE" "$VOSK_LIB_DIR/"
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        sudo ldconfig
    fi
    
    # Clean up
    cd - > /dev/null
    rm -rf "$TEMP_DIR"
    
    echo "✅ Vosk library installed to $VOSK_LIB_DIR"
fi

echo ""
echo "🎉 Scriba has been successfully installed!"
echo ""
echo "Usage:"
echo "  scriba                  # Start with interactive model selection"
echo "  scriba --no-typing      # Transcription only, no automatic typing"
echo "  scriba --debug          # Show debug output"
echo "  scriba --select-model   # Force model selection"
echo "  scriba --help           # Show all options"
echo ""
echo "Configuration will be stored in ~/.config/scriba/"
echo ""
echo "🎙️  Ready to transcribe! Run 'scriba' to get started."
