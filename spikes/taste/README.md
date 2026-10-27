# Personal taste evaluation

Measures how well the personal taste model (`crates/core/src/taste.rs`,
#218) learns a preference from picks and rejects. Results:
[results.md](results.md).

It reuses the sample of the aesthetic spike (`spikes/aesthetic`): the
first folder plays the user's picks, the second their rejects.

```sh
cargo build --release -p picturesque-infra --example eval_taste
LD_LIBRARY_PATH=target/release target/release/examples/eval_taste \
    model.onnx spikes/aesthetic/sample/featured spikes/aesthetic/sample/ordinary \
    [embeddings cache]
```
