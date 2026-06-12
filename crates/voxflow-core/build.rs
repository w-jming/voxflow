fn main() {
    // voxflow-asr-sherpa links the prebuilt sherpa-onnx shared libraries that
    // cargo copies next to the binaries; rpath must be set on the final bin.
    println!("cargo:rustc-link-arg-bins=-Wl,-rpath,$ORIGIN");
}
