//! Lets the helper find llama.cpp's shared libraries beside it
//! (`scripts/critic-libs.sh`, ADR-0016).

fn main() {
    match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("linux") => println!("cargo:rustc-link-arg-bins=-Wl,-rpath,$ORIGIN"),
        Ok("macos") => {
            println!("cargo:rustc-link-arg-bins=-Wl,-rpath,@executable_path");
            println!("cargo:rustc-link-arg-bins=-Wl,-rpath,@executable_path/../Frameworks");
        }
        // Windows looks beside the executable.
        _ => {}
    }
}
