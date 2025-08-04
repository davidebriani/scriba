{
  description = "Scriba - Real-time speech transcription tool for software developers";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        
        # Supported architectures for cross-compilation
        # Note: Vosk library availability may limit actual support
        supportedSystems = [
          "x86_64-linux"
          "aarch64-linux" 
          "x86_64-darwin"
          "aarch64-darwin"
        ];

        rustVersion = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "clippy" "rustfmt" ];
        };

        # Native dependencies required for building
        nativeDeps = with pkgs; [
          pkg-config
          cmake
          rustVersion
        ];

        # Runtime dependencies
        runtimeDeps = with pkgs; [
          alsa-lib          # ALSA sound system (Linux)
          libpulseaudio     # PulseAudio (Linux)
          xorg.libX11       # X11 for enigo keyboard simulation
          xorg.libXtst      # X11 testing extension
          xorg.libXi        # X11 input extension
          libxkbcommon      # Keyboard handling
          openssl           # SSL/TLS support
          curl              # HTTP client library
          unzip             # For extracting models
          xdotool           # For xdo library (keyboard simulation)
        ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
          # macOS-specific dependencies
          darwin.apple_sdk.frameworks.CoreAudio
          darwin.apple_sdk.frameworks.AudioUnit
          darwin.apple_sdk.frameworks.CoreServices
          darwin.apple_sdk.frameworks.ApplicationServices
        ];

        # Vosk library setup
        voskLib = pkgs.stdenv.mkDerivation rec {
          pname = "vosk";
          version = "0.3.45";
          
          src = pkgs.fetchurl {
            url = if pkgs.stdenv.isLinux then
              (if pkgs.stdenv.isAarch64 then
                "https://github.com/alphacep/vosk-api/releases/download/v${version}/vosk-linux-aarch64-${version}.zip"
              else
                "https://github.com/alphacep/vosk-api/releases/download/v${version}/vosk-linux-x86_64-${version}.zip")
            else if pkgs.stdenv.isDarwin then
              "https://github.com/alphacep/vosk-api/releases/download/v${version}/vosk-osx-${version}.zip"
            else
              throw "Unsupported platform for Vosk";
            sha256 = if pkgs.stdenv.isLinux then
              (if pkgs.stdenv.isAarch64 then
                "sha256-ReWdN3Vd2wdWjnlJfX/rqMA67lqeBx3ymWGqAj/ZRUE="  # vosk-linux-aarch64-0.3.45.zip
              else
                "sha256-u9yO2FxDl59kQxQoiXcOqVy/vFbP+1xdzXOvqHXF+7I=")  # vosk-linux-x86_64-0.3.45.zip
            else
              "sha256-ABnfxLMtY8E5KqJkrtIlPB4MLLCRb44swyabbLi7SbU=";     # vosk-osx-0.3.45.zip
          };

          nativeBuildInputs = [ pkgs.unzip ];
          
          unpackPhase = ''
            unzip $src
            echo "=== Extracted files ==="
            find . -type f | sort
            echo "======================"
          '';
          
          installPhase = ''
            mkdir -p $out/lib $out/include
            
            # Copy libraries - be more explicit about what we're looking for
            echo "=== Looking for libraries ==="
            find . -name "*vosk*" -type f
            echo "============================="
            
            # Copy libraries
            if [[ -f vosk-*/libvosk.so ]]; then
              cp vosk-*/libvosk.so* $out/lib/
              echo "Copied libvosk.so"
            elif [[ -f vosk-*/lib/libvosk.so ]]; then
              cp vosk-*/lib/libvosk.so* $out/lib/
              echo "Copied lib/libvosk.so"
            elif ls vosk-*/libvosk.so* 1> /dev/null 2>&1; then
              cp vosk-*/libvosk.so* $out/lib/
              echo "Copied libvosk.so* pattern"
            else
              echo "WARNING: Could not find libvosk.so"
              find . -name "*vosk*" -type f
            fi
            
            # Copy headers
            if [[ -f vosk-*/vosk_api.h ]]; then
              cp vosk-*/vosk_api.h $out/include/
              echo "Copied vosk_api.h"
            elif [[ -f vosk-*/include/vosk_api.h ]]; then
              cp vosk-*/include/vosk_api.h $out/include/
              echo "Copied include/vosk_api.h"
            else
              echo "WARNING: Could not find vosk_api.h"
            fi
            
            # Set up pkg-config
            mkdir -p $out/lib/pkgconfig
            cat > $out/lib/pkgconfig/vosk.pc << EOF
            prefix=$out
            exec_prefix=\''${prefix}
            libdir=\''${exec_prefix}/lib
            includedir=\''${prefix}/include

            Name: vosk
            Description: Vosk speech recognition library
            Version: ${version}
            Libs: -L\''${libdir} -lvosk
            Cflags: -I\''${includedir}
            EOF
            
            # Show what we have
            echo "=== Installed files ==="
            find $out -type f | sort
            echo "======================"
          '';
        };

        # Main package derivation
        scriba = pkgs.rustPlatform.buildRustPackage rec {
          pname = "scriba";
          version = "0.1.0";

          src = ./.;

          cargoHash = "sha256-ke6T1vhpnm4pTemNocT832gn1Pvg5r3CztH3gAL9zFc=";

          nativeBuildInputs = nativeDeps ++ [ voskLib ];
          buildInputs = runtimeDeps ++ [ voskLib ];

          # Environment variables for build
          VOSK_LIBRARY_PATH = "${voskLib}/lib";
          PKG_CONFIG_PATH = "${voskLib}/lib/pkgconfig:${pkgs.lib.makeSearchPathOutput "dev" "lib/pkgconfig" runtimeDeps}";

          # Make sure Cargo can find the Vosk library
          LIBRARY_PATH = pkgs.lib.makeLibraryPath [ voskLib pkgs.xdotool ];
          RUSTFLAGS = "-L native=${voskLib}/lib -L native=${pkgs.xdotool}/lib";
          
          # For the vosk-sys crate specifically
          VOSK_STATIC = "0";  # Use dynamic linking
          VOSK_LIB_DIR = "${voskLib}/lib";
          VOSK_INCLUDE_DIR = "${voskLib}/include";

          # Skip tests for now as they may require audio devices
          doCheck = false;

          # Runtime library path setup
          preFixup = pkgs.lib.optionalString pkgs.stdenv.isLinux ''
            patchelf --set-rpath "${pkgs.lib.makeLibraryPath (runtimeDeps ++ [ voskLib ])}" $out/bin/scriba
          '';

          meta = with pkgs.lib; {
            description = "Real-time speech transcription tool focused on software engineering terms";
            homepage = "https://github.com/davidebriani/scriba";
            license = licenses.mit;
            maintainers = [ ];
            platforms = supportedSystems;
          };
        };

      in
      {
        packages = {
          default = scriba;
          scriba = scriba;
        };

        # Development shell
        devShells.default = pkgs.mkShell {
          buildInputs = nativeDeps ++ runtimeDeps ++ [ voskLib ];
          
          shellHook = ''
            export VOSK_LIBRARY_PATH="${voskLib}/lib"
            export PKG_CONFIG_PATH="${voskLib}/lib/pkgconfig:$PKG_CONFIG_PATH"
            export LD_LIBRARY_PATH="${voskLib}/lib:$LD_LIBRARY_PATH"
            
            echo "🎙️  Scriba development environment"
            echo "Vosk library: ${voskLib}/lib"
            echo "Run 'cargo build' to build the project"
          '';
        };

        # Apps for easy running
        apps.default = {
          type = "app";
          program = "${scriba}/bin/scriba";
          meta = with pkgs.lib; {
            description = "Real-time speech transcription tool focused on software engineering terms";
          };
        };
        
        apps.scriba = {
          type = "app";
          program = "${scriba}/bin/scriba";
          meta = with pkgs.lib; {
            description = "Real-time speech transcription tool focused on software engineering terms";
          };
        };
      });
}
