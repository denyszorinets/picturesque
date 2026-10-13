"""Writes tiny_clip.onnx: a stand-in for the CLIP image encoder with the
same input and output ("image" [1, 3, 224, 224] -> "embedding" [1, 768]).

The embedding is the image's mean colour projected to 768 dimensions, so
photos of similar colour get similar embeddings. Used by the inference
integration tests. Regenerate with:
    uv run --no-project --with onnx==1.19.0 --with numpy==2.3.3 python crates/infra/tests/fixtures/onnx/make_tiny_clip.py
"""

from pathlib import Path

import numpy as np
import onnx
from onnx import TensorProto, helper, numpy_helper

rng = np.random.default_rng(42)
projection = rng.standard_normal((3, 768)).astype(np.float32)

image = helper.make_tensor_value_info("image", TensorProto.FLOAT, [1, 3, 224, 224])
embedding = helper.make_tensor_value_info("embedding", TensorProto.FLOAT, [1, 768])
graph = helper.make_graph(
    [
        helper.make_node("GlobalAveragePool", ["image"], ["pooled"]),
        helper.make_node("Flatten", ["pooled"], ["colour"]),
        helper.make_node("MatMul", ["colour", "projection"], ["embedding"]),
    ],
    "tiny_clip",
    [image],
    [embedding],
    initializer=[numpy_helper.from_array(projection, "projection")],
)
model = helper.make_model(
    graph,
    producer_name="picturesque-tests",
    opset_imports=[helper.make_opsetid("", 17)],
)
model.ir_version = 8
onnx.checker.check_model(model)
onnx.save(model, Path(__file__).with_name("tiny_clip.onnx"))
