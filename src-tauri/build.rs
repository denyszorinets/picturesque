fn main() {
    // ONNX Runtime's WebGPU provider needs libwebgpu_dawn.so, copied next to
    // the executable; let the executable find it there.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        println!("cargo:rustc-link-arg-bins=-Wl,-rpath,$ORIGIN");
    }
    tauri_build::build();
}
