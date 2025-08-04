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
          '';
          
          installPhase = ''
            mkdir -p $out/lib $out/include
            
            # Copy libraries
            if [[ -f vosk-*/libvosk.so ]]; then
              cp vosk-*/libvosk.so* $out/lib/
            elif [[ -f vosk-*/libvosk.dylib ]]; then
              cp vosk-*/libvosk.dylib $out/lib/
            fi
            
            # Copy headers
            if [[ -d vosk-*/vosk_api.h ]]; then
              cp vosk-*/vosk_api.h $out/include/
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
          '';
        };

        # Main package derivation
        scriba = pkgs.rustPlatform.buildRustPackage rec {
          pname = "scriba";
          version = "0.1.0";

          src = ./.;

          cargoHash = "sha256-TQrnsaZSRVmgSuYdTq+pNCG4gHa5RbA2OgBeTgX4y14=";

          nativeBuildInputs = nativeDeps ++ [ voskLib ];
          buildInputs = runtimeDeps ++ [ voskLib ];

          # Environment variables for build
          VOSK_LIBRARY_PATH = "${voskLib}/lib";
          PKG_CONFIG_PATH = "${voskLib}/lib/pkgconfig:${pkgs.lib.makeSearchPathOutput "dev" "lib/pkgconfig" runtimeDeps}";

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
