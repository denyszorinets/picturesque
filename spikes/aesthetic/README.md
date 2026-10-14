# Aesthetic score evaluation

Measures how well candidate aesthetic predictors separate photos people
rated highly from ordinary ones (#44). Results: [results.md](results.md).

1. `python3 scripts/fetch_commons.py 150` downloads the sample into
   `sample/` (not committed): Wikimedia Commons *Featured pictures* (chosen
   by community vote) and random Commons camera photos.
2. Download a CLIP image encoder (`visual/model.onnx` from the
   `immich-app/ViT-*__openai` repositories on Hugging Face).
3. Run the evaluation, optionally with another head (converted with
   `crates/app/src/analysis/aesthetic/convert.py` or similar):

```sh
cargo build --release -p picturesque-infra --example eval_aesthetic
LD_LIBRARY_PATH=target/release target/release/examples/eval_aesthetic \
    model.onnx spikes/aesthetic/sample/featured spikes/aesthetic/sample/ordinary [head.bin]
```
