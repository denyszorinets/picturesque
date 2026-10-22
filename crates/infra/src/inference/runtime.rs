//! Where ONNX Runtime comes from, and whether it can run here (#154).
//!
//! On Linux x86-64 the runtime is a shared library, `libonnxruntime.so`,
//! linked from pyke's static build by `scripts/ort-dylib.sh` and loaded the
//! first time a model is needed, never when the app starts. pyke builds it
//! for x86-64-v3 (Intel Haswell, AMD Excavator and later): its static
//! initializers alone use BMI2, so loading it on an older processor would
//! end the process with "Illegal instruction". The processor is checked
//! first; without the instructions, AI is unavailable and the rest of the
//! app works.
//!
//! On other targets the runtime is linked into the executable and always
//! available.

/// Whether models can run here: `Err` with the reason when they cannot.
/// Decided once, the first time it is asked.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub fn ensure() -> Result<(), String> {
    use std::sync::OnceLock;

    static READY: OnceLock<Result<(), String>> = OnceLock::new();
    READY.get_or_init(load).clone()
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
pub fn ensure() -> Result<(), String> {
    Ok(())
}

/// Instructions of x86-64-v3 that the processor lacks.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn missing_instructions() -> Vec<&'static str> {
    let mut missing = Vec::new();
    macro_rules! need {
        ($($feature:tt),*) => {
            $(if !std::is_x86_feature_detected!($feature) { missing.push($feature); })*
        };
    }
    need!(
        "avx", "avx2", "bmi1", "bmi2", "fma", "f16c", "lzcnt", "movbe"
    );
    missing
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn load() -> Result<(), String> {
    let missing = missing_instructions();
    if !missing.is_empty() {
        return Err(format!(
            "AI features need a processor with AVX2 (Intel Haswell or AMD Excavator, 2013 \
             or later); this one lacks {}. Everything else works.",
            missing.join(", ").to_uppercase()
        ));
    }
    let path = locate().ok_or_else(|| {
        "ONNX Runtime (libonnxruntime.so) was not found next to the app; reinstall \
         Picturesque, or set PICTURESQUE_ORT_DYLIB to its path."
            .to_owned()
    })?;
    ort::init_from(&path)
        .map_err(|e| format!("could not load {}: {e}", path.display()))?
        .commit();
    Ok(())
}

/// `PICTURESQUE_ORT_DYLIB`, or `libonnxruntime.so` beside the executable:
/// next to it (development builds), one folder up (tests and examples in
/// `target/<profile>/deps` and `examples`), or in `../lib/picturesque`
/// (deb, rpm and AppImage).
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn locate() -> Option<std::path::PathBuf> {
    use std::path::PathBuf;

    const NAME: &str = "libonnxruntime.so";
    if let Some(path) = std::env::var_os("PICTURESQUE_ORT_DYLIB").filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(path));
    }
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    [
        dir.join(NAME),
        dir.join("..").join(NAME),
        dir.join("../lib/picturesque").join(NAME),
    ]
    .into_iter()
    .find(|p| p.is_file())
}

#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
mod tests {
    use super::*;

    #[test]
    fn this_processor_runs_the_runtime() {
        // CI and development machines are x86-64-v3; the check must not
        // reject them.
        assert_eq!(missing_instructions(), Vec::<&str>::new());
    }
}
