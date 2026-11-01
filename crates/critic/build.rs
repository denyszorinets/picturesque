//! Lets the helper find llama.cpp's shared libraries beside it
//! (`scripts/critic-libs.sh`, ADR-0016), and tidies after llama-cpp-sys-2.

use std::path::Path;

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
    remove_dangling_library_links();
}

/// llama-cpp-sys-2 hard-links every `lib*.so*` it installed into
/// `target/<profile>`, `deps` and `examples`, including the symbolic links
/// (`libllama.so -> libllama.so.0`), which dangle there. The next build of
/// it (`cargo test` after `cargo clippy`) takes a dangling link for a
/// missing file, links again and fails with *File exists*. This build
/// script runs after it, so removing them here keeps the next build working.
fn remove_dangling_library_links() {
    let Some(out) = std::env::var_os("OUT_DIR") else {
        return;
    };
    // OUT_DIR is target/<profile>/build/<crate>-<hash>/out.
    let Some(profile) = Path::new(&out).ancestors().nth(3) else {
        return;
    };
    for dir in [
        profile.to_path_buf(),
        profile.join("deps"),
        profile.join("examples"),
    ] {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let dangling = path.symlink_metadata().is_ok_and(|m| m.is_symlink()) && !path.exists();
            if dangling && name.starts_with("lib") && name.contains(".so") {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
}
