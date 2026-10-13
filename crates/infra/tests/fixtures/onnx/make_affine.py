"""Writes affine.onnx: y = 2 * x + 1 for a float tensor x of shape [N, 3].

Used by the inference tests, and built into the app as the provider probe
(src/inference/probe.onnx). Regenerate both with:
    uv run --no-project --with onnx==1.19.0 python crates/infra/tests/fixtures/onnx/make_affine.py
"""

from pathlib import Path

import onnx
from onnx import TensorProto, helper

x = helper.make_tensor_value_info("x", TensorProto.FLOAT, ["n", 3])
y = helper.make_tensor_value_info("y", TensorProto.FLOAT, ["n", 3])
two = helper.make_tensor("two", TensorProto.FLOAT, [], [2.0])
one = helper.make_tensor("one", TensorProto.FLOAT, [], [1.0])
graph = helper.make_graph(
    [
        helper.make_node("Mul", ["x", "two"], ["scaled"]),
        helper.make_node("Add", ["scaled", "one"], ["y"]),
    ],
    "affine",
    [x],
    [y],
    initializer=[two, one],
)
model = helper.make_model(
    graph,
    producer_name="picturesque-tests",
    opset_imports=[helper.make_opsetid("", 17)],
)
model.ir_version = 8
onnx.checker.check_model(model)
here = Path(__file__).parent
onnx.save(model, here / "affine.onnx")
onnx.save(model, here.parents[2] / "src" / "inference" / "probe.onnx")
