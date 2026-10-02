// The native reference draws with the host build of the core's own Mesa
// (waterbox/setup-mesa.sh -n), never with whatever OpenGL the host has: the
// OpenGL is part of the machine, and the reference must be the same machine
// as the sandbox.
use std::path::PathBuf;

fn main() {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mesa = std::env::var("MESA_NATIVE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| here.join("../../build/mesa/build-native"));
    let osmesa = mesa.join("src/gallium/targets/osmesa");
    if !osmesa.join("libOSMesa.so").exists() {
        panic!(
            "no host Mesa at {} - run waterbox/setup-mesa.sh -n first",
            osmesa.display()
        );
    }
    let osmesa = osmesa.canonicalize().unwrap();
    println!("cargo:rustc-link-search=native={}", osmesa.display());
    println!("cargo:rustc-link-lib=dylib=OSMesa");
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", osmesa.display());
    println!("cargo:rerun-if-env-changed=MESA_NATIVE_DIR");
}
