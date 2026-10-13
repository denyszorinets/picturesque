"""Writes tiny_clip_text.onnx: a stand-in for the CLIP text encoder with the
same input and output ("text" int32 [1, 77] -> "embedding" [1, 768]): the
first three token ids as floats, projected to 768 dimensions. Used by the search
integration tests. Regenerate with:
    uv run --no-project --with onnx==1.19.0 --with numpy==2.3.3 python crates/infra/tests/fixtures/onnx/make_tiny_clip_text.py
"""

from pathlib import Path

import numpy as np
import onnx
from onnx import TensorProto, helper, numpy_helper

rng = np.random.default_rng(43)
projection = rng.standard_normal((3, 768)).astype(np.float32)

text = helper.make_tensor_value_info("text", TensorProto.INT32, [1, 77])
embedding = helper.make_tensor_value_info("embedding", TensorProto.FLOAT, [1, 768])
graph = helper.make_graph(
    [
        helper.make_node("Slice", ["text", "starts", "ends", "axes"], ["first"]),
        helper.make_node("Cast", ["first"], ["ids"], to=TensorProto.FLOAT),
        helper.make_node("MatMul", ["ids", "projection"], ["embedding"]),
    ],
    "tiny_clip_text",
    [text],
    [embedding],
    initializer=[
        numpy_helper.from_array(projection, "projection"),
        numpy_helper.from_array(np.array([0], dtype=np.int64), "starts"),
        numpy_helper.from_array(np.array([3], dtype=np.int64), "ends"),
        numpy_helper.from_array(np.array([1], dtype=np.int64), "axes"),
    ],
)
model = helper.make_model(
    graph,
    producer_name="picturesque-tests",
    opset_imports=[helper.make_opsetid("", 17)],
)
model.ir_version = 8
onnx.checker.check_model(model)
onnx.save(model, Path(__file__).with_name("tiny_clip_text.onnx"))
