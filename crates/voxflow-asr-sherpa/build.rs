fn main() {
    // sherpa-rs-sys copies libsherpa-onnx-c-api.so / libonnxruntime.so next to
    // the produced binaries; $ORIGIN lets them run outside `cargo run` too.
    println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN");
}
