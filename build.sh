#!/bin/bash

# Build script for Scriba - Real-time Speech Transcription Tool
# Usage: ./build.sh [clean|release|debug|install]

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

function print_usage() {
    echo "Usage: $0 [command]"
    echo ""
    echo "Commands:"
    echo "  clean    - Clean build artifacts"
    echo "  debug    - Build in debug mode"
    echo "  release  - Build in release mode (default)"
    echo "  install  - Build and install to ~/.local/bin"
    echo "  help     - Show this help"
    echo ""
    echo "Environment variables:"
    echo "  VOSK_LIBRARY_PATH - Path to Vosk library directory"
}

function clean_build() {
    echo -e "${BLUE}🧹 Cleaning build artifacts...${NC}"
    cargo clean
    echo -e "${GREEN}✅ Clean complete${NC}"
}

function build_debug() {
    echo -e "${BLUE}🔨 Building Scriba in debug mode...${NC}"
    cargo build
    echo -e "${GREEN}✅ Debug build complete${NC}"
    echo -e "${YELLOW}Binary located at: target/debug/scriba${NC}"
}

function build_release() {
    echo -e "${BLUE}🔨 Building Scriba in release mode...${NC}"
    cargo build --release
    echo -e "${GREEN}✅ Release build complete${NC}"
    echo -e "${YELLOW}Binary located at: target/release/scriba${NC}"
}

function install_binary() {
    echo -e "${BLUE}📦 Installing Scriba...${NC}"
    
    # Build in release mode first
    build_release
    
    # Create ~/.local/bin if it doesn't exist
    mkdir -p ~/.local/bin
    
    # Copy binary
    cp target/release/scriba ~/.local/bin/
    chmod +x ~/.local/bin/scriba
    
    echo -e "${GREEN}✅ Scriba installed to ~/.local/bin/scriba${NC}"
    
    # Check if ~/.local/bin is in PATH
    if [[ ":$PATH:" != *":$HOME/.local/bin:"* ]]; then
        echo -e "${YELLOW}⚠️  Warning: ~/.local/bin is not in your PATH${NC}"
        echo -e "${YELLOW}   Add this line to your ~/.bashrc or ~/.zshrc:${NC}"
        echo -e "${YELLOW}   export PATH=\"\$HOME/.local/bin:\$PATH\"${NC}"
    fi
}

# Parse command line arguments
case "${1:-release}" in
    "clean")
        clean_build
        ;;
    "debug")
        build_debug
        ;;
    "release")
        build_release
        ;;
    "install")
        install_binary
        ;;
    "help")
        print_usage
        ;;
    *)
        echo -e "${RED}❌ Unknown command: $1${NC}"
        echo ""
        print_usage
        exit 1
        ;;
esac
