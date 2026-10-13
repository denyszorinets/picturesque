# Best-in-series ranking: evaluation on a real shoot

#47 asks for a measured improvement of the AI ranking (technical scores,
aesthetic score, faces) over the M3 ranking (technical scores only). This
needs a real shoot with your own choices:

1. Put each series in its own folder, as JPEGs (export RAWs first), and
   start the file name of the photo you consider best with `best`:

   ```
   shoot/
     01-portrait/   best_IMG_1043.jpg  IMG_1041.jpg  IMG_1042.jpg  IMG_1044.jpg
     02-dog/        IMG_1101.jpg  best_IMG_1102.jpg  IMG_1103.jpg
   ```

2. Download the models (in the app, or the files listed in
   `crates/app/src/models.rs`) and run:

   ```sh
   cargo build --release -p picturesque-infra --example eval_ranking
   LD_LIBRARY_PATH=target/release target/release/examples/eval_ranking \
       shoot/ clip-visual.onnx face_detection_yunet_2023mar.onnx face_mesh.onnx
   ```

It prints where each ranking placed your pick and how often each put it
first. Results: pending a real shoot.

A smoke test on four fixture photos (open versus closed eyes, not real
series) placed the open-eyed photo first in 2 of 2 pairs with the AI
signals and 1 of 2 with technical scores only.
