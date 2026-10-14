"""Converts the improved LAION aesthetic predictor to one linear head.

Source: https://github.com/christophschuhmann/improved-aesthetic-predictor
(Apache-2.0), file sac+logos+ava1-l14-linearMSE.pth (sha256 21dd590f...f1e0):
for CLIP ViT-L/14 embeddings, nn.Linear layers 768 -> 1024 -> 128 -> 64 ->
16 -> 1 with only dropout between them, so in inference they compose into
a single linear map. Writes vit_l_14.bin: 768 little-endian f32 weights
followed by the f32 bias. Reads the checkpoint without torch:

    curl -sLo improved.pth "https://github.com/christophschuhmann/improved-aesthetic-predictor/raw/main/sac+logos+ava1-l14-linearMSE.pth"
    uv run --no-project --with numpy==2.3.3 python crates/app/src/analysis/aesthetic/convert.py improved.pth
"""

import collections
import io
import pickle
import sys
import zipfile
from pathlib import Path

import numpy as np

checkpoint = zipfile.ZipFile(sys.argv[1])
root = checkpoint.namelist()[0].split("/")[0]


class Storage:
    def __init__(self, key):
        self.key = key


def rebuild_tensor(storage, offset, size, stride, *rest):
    data = np.frombuffer(checkpoint.read(f"{root}/data/{storage.key}"), dtype="<f4")
    return data[offset : offset + int(np.prod(size))].reshape(size)


class StateDict(pickle.Unpickler):
    def find_class(self, module, name):
        if name == "_rebuild_tensor_v2":
            return rebuild_tensor
        if name == "OrderedDict":
            return collections.OrderedDict
        return lambda *args, **kwargs: None

    def persistent_load(self, pid):
        return Storage(pid[2])


state = StateDict(io.BytesIO(checkpoint.read(f"{root}/data.pkl"))).load()
layers = sorted({k.rsplit(".", 1)[0] for k in state}, key=lambda k: int(k.split(".")[1]))
weight, bias = None, None
for layer in layers:
    w = state[f"{layer}.weight"].astype(np.float64)
    b = state[f"{layer}.bias"].astype(np.float64)
    weight, bias = (w, b) if weight is None else (w @ weight, w @ bias + b)
assert weight.shape == (1, 768)
out = Path(__file__).with_name("vit_l_14.bin")
out.write_bytes(weight.astype("<f4").tobytes() + bias.astype("<f4").tobytes())
