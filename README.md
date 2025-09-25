# 🎙️ Scriba

**Real-time speech transcription tool**

Scriba is a cross-platform speech recognition tool that transcribes audio in real-time and can automatically type the recognized text.

## ✨ Features

- **Real-time transcription** using the [Vosk](https://github.com/alphacep/vosk-api) speech recognition engine
- **25+ language support** including English, Chinese, Russian, German, French, Spanish, Japanese, and many more
- **Software engineering optimization** - converts spoken numbers to digits and recognizes programming terms
- **Automatic typing** - simulates keyboard input to type transcribed text wherever you focus
- **Cross-platform support** - Linux, macOS, and Windows
- **Multiple model sizes** - from compact 30MB models to high-accuracy 2GB+ models
- **Interactive model selection** with automatic downloading
- **Configurable confidence thresholds** to filter out uncertain transcriptions
- **XDG-compliant configuration** - stores models and settings in `~/.config/scriba/`

## 🚀 Installation

### Option 1: Download Pre-built Binaries

Download the latest release for your platform from the [GitHub releases page](https://github.com/davidebriani/scriba/releases):

- `scriba-linux-x86_64.tar.gz` - Linux (Intel/AMD 64-bit)
- `scriba-linux-aarch64.tar.gz` - Linux (ARM 64-bit, e.g., Raspberry Pi)
- `scriba-macos-x86_64.tar.gz` - macOS (Intel)
- `scriba-macos-aarch64.tar.gz` - macOS (Apple Silicon)
- `scriba-windows-x86_64.zip` - Windows (64-bit)

Extract the archive and run the `scriba` binary directly.

### Option 2: Using Nix (Recommended for Nix users)

```bash
# Run directly
nix run github:davidebriani/scriba

# Install to profile
nix profile install github:davidebriani/scriba

# Use in a development shell
nix develop github:davidebriani/scriba
```

### Option 3: Build from Source

#### Prerequisites

**Linux (Ubuntu/Debian):**
```bash
sudo apt-get install libasound2-dev libpulse-dev libxdo-dev pkg-config build-essential curl unzip
```

**Linux (Fedora/RHEL):**
```bash
sudo dnf install alsa-lib-devel pulseaudio-libs-devel libxdo-devel pkg-config gcc curl unzip
```

**macOS:**
```bash
# Install Xcode command line tools
xcode-select --install
```

**Windows:**
```bash
# Install Visual Studio Build Tools or Visual Studio Community
# Rust will automatically detect and use the MSVC toolchain
```

#### Build Instructions

1. **Clone the repository:**
   ```bash
   git clone https://github.com/davidebriani/scriba.git
   cd scriba
   ```

2. **Install Rust** (if not already installed):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source $HOME/.cargo/env
   ```

3. **Build and install:**
   ```bash
   ./install.sh
   ```

   This script will:
   - Build Scriba in release mode
   - Download and install the Vosk native library
   - Install the binary to `~/.local/bin/scriba`
   - Set up the environment

## 🎯 Usage

### Basic Usage

```bash
# Start with interactive model selection
scriba

# Transcription only (no automatic typing)
scriba --no-typing

# Enable debug output to see partial transcriptions
scriba --debug

# Use a specific confidence threshold (0.0-1.0)
scriba --confidence-threshold 0.8

# Force model selection even if a model exists
scriba --select-model

# Show all options
scriba --help
```

### Configuration

Scriba stores its configuration and downloaded models in:
- **Linux/macOS**: `~/.config/scriba/`
- **Windows**: `%APPDATA%\scriba\`

The first time you run Scriba, you'll be prompted to select a speech recognition model. Models are automatically downloaded and cached for future use.

### Available Models

Scriba supports 25+ languages with different model sizes:

#### English
- **Small English US** (40MB) - Fast, basic vocabulary
- **English US (Recommended)** (128MB) - Best balance of speed and accuracy
- **Large English US** (1.8GB) - Highest accuracy
- **English US (GigaSpeech)** (2.3GB) - Latest model with improved accuracy
- **English India** (1GB) - Optimized for Indian accents

#### Other Languages
- **Chinese** (Standard & Small)
- **Russian** (Standard & Small)
- **German** (Standard & Small)
- **French** (Small)
- **Spanish** (Standard & Small)
- **Portuguese** (Standard & Small)
- **Italian** (Standard & Small)
- **Dutch** (Standard & Small)
- **Japanese** (Standard & Small)
- **Korean** (Small)
- **Hindi** (Standard & Small)
- **Ukrainian** (Standard & Small)
- **Arabic**, **Persian**, **Turkish**, **Vietnamese**, **Polish**, **Gujarati**

### Software Engineering Features

Scriba automatically converts spoken programming terms:

- **Numbers**: "one thousand twenty five" → "1025"
- **Digits**: "zero through nine" → "0" through "9"
- **Programming terms**:
  - "open paren" → "("
  - "close paren" → ")"
  - "open bracket" → "["
  - "close bracket" → "]"
  - "open brace" → "{"
  - "close brace" → "}"
  - "semicolon" → ";"
  - "equals" → "="
  - "null" → "null"
  - "true" → "true"
  - "false" → "false"

## 🔧 Development

### Building with Nix

```bash
# Enter development shell
nix develop

# Build the project
cargo build

# Run tests
cargo test

# Build for release
cargo build --release
```

### Manual Development Setup

1. **Install dependencies** (see Prerequisites above)

2. **Set up Vosk library**:
   ```bash
   # The build script will help you set this up
   ./build.sh
   ```

3. **Build and run**:
   ```bash
   cargo build
   cargo run -- --help
   ```

### Cross-compilation

The project supports cross-compilation for multiple architectures. See the GitHub Actions workflow (`.github/workflows/release.yml`) for examples of building for different targets.

## 🏗️ Architecture

- **Audio Capture**: Uses `cpal` for cross-platform audio input
- **Speech Recognition**: Vosk engine with downloadable models
- **Text Processing**: Enhanced number and programming term conversion
- **Keyboard Simulation**: `enigo` for cross-platform input simulation
- **Async Runtime**: Tokio for concurrent audio processing and transcription
- **CLI Interface**: `clap` for argument parsing and `dialoguer` for interactive prompts

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request. For major changes, please open an issue first to discuss what you would like to change.

### Development Guidelines

1. Follow the existing code style
2. Add tests for new functionality
3. Update documentation as needed
4. Ensure cross-platform compatibility

## 📝 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- [Vosk](https://alphacephei.com/vosk/) - Open source speech recognition toolkit
- [cpal](https://github.com/RustAudio/cpal) - Cross-platform audio I/O library
- [enigo](https://github.com/enigo-rs/enigo) - Cross-platform input simulation
- [Tokio](https://tokio.rs/) - Asynchronous runtime for Rust

## 🐛 Troubleshooting

### Linux Issues

**"No input device available"**
```bash
# Check audio devices
arecord -l

# Install ALSA/PulseAudio development packages
sudo apt-get install libasound2-dev libpulse-dev
```

**"libvosk.so not found"**
```bash
# Run the install script to set up Vosk library
./install.sh

# Or manually set the library path
export LD_LIBRARY_PATH=/usr/local/lib:$LD_LIBRARY_PATH
```

### macOS Issues

**Permission denied for microphone**
- Go to System Preferences → Security & Privacy → Privacy → Microphone
- Add Terminal or your terminal application to the allowed list

### Windows Issues

**"The system cannot find the specified module"**
- Ensure `libvosk.dll` is in the same directory as `scriba.exe` or in your system PATH
- Install Visual C++ Redistributable if needed

---

For more help, please [open an issue](https://github.com/davidebriani/scriba/issues) on GitHub.
