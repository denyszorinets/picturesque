# Built-in critic spike: Qwen3-VL through llama.cpp

Throwaway spike for running photo critiques in-process with llama.cpp
(`llama-cpp-2` with its multimodal `mtmd` module) instead of a separate
Ollama server (ADR-0016). Not part of the Cargo workspace and not
maintained. Results: [results.md](results.md).

Needs CMake and a C++ compiler; the Vulkan build also needs the Vulkan
headers and loader, `glslc` and SPIRV-Headers (Fedora: `cmake glslc
vulkan-headers vulkan-loader-devel spirv-headers-devel`).

Model files (Apache-2.0), from Qwen's official repository, pinned:

```sh
R=https://huggingface.co/Qwen/Qwen3-VL-4B-Instruct-GGUF/resolve/1cd86afb9a95c410a6038ab3b40d8b578c892266
curl -L -o model.gguf  $R/Qwen3VL-4B-Instruct-Q4_K_M.gguf
curl -L -o mmproj.gguf $R/mmproj-Qwen3VL-4B-Instruct-Q8_0.gguf
```

```sh
cargo run --release -- model.gguf mmproj.gguf photo.jpg                       # CPU
cargo run --release --features vulkan -- model.gguf mmproj.gguf photo.jpg 99  # GPU
```

The photo should be the 768 px preview the app sends
(`vipsthumbnail photo.jpg -s 768 -o small.jpg[Q=85,strip]`).
