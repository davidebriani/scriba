use std::env;

fn main() {
    // Check for VOSK_LIBRARY_PATH environment variable
    if let Ok(lib_path) = env::var("VOSK_LIBRARY_PATH") {
        println!("cargo:rustc-link-search=native={}", lib_path);
    } else {
        // Default search paths for Linux
        println!("cargo:rustc-link-search=native=/usr/local/lib");
        println!("cargo:rustc-link-search=native=/usr/lib");
        println!("cargo:rustc-link-search=native=/usr/lib/x86_64-linux-gnu");
    }
    
    // Link to vosk library
    println!("cargo:rustc-link-lib=dylib=vosk");
    
    // Re-run build script if environment variable changes
    println!("cargo:rerun-if-env-changed=VOSK_LIBRARY_PATH");
}
