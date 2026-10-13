fn main() {
    // ONNX Runtime's WebGPU provider needs libwebgpu_dawn.so: next to the
    // executable in development builds and AppImages, in
    // /usr/lib/picturesque in deb and rpm packages (tauri.conf.json).
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        println!("cargo:rustc-link-arg-bins=-Wl,-rpath,$ORIGIN:$ORIGIN/../lib/picturesque");
    }
    tauri_build::build();
}
