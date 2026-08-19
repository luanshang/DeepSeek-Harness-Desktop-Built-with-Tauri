use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    tauri_build::build();

    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let payload = manifest.join("../installer/payload.exe");
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("cargo OUT_DIR")).join("payload.exe");
    if !payload.exists() {
        panic!("installer payload is missing: {} (run `npm run build:installer` from the project root)", payload.display());
    }
    fs::copy(&payload, &out).expect("failed to stage installer payload");
    println!("cargo:rerun-if-changed={}", payload.display());
}
