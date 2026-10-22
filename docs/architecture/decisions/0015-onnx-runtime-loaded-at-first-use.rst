ADR-0015: ONNX Runtime Loaded at First Use on Linux
===================================================

:Status: Accepted
:Date: 2026-10-21

Context
-------

:doc:`ADR-0006 <0006-local-ai-with-onnx-runtime>` uses the prebuilt ONNX
Runtime that ``ort`` downloads from pyke, linked statically into the
executable, with Dawn (``libwebgpu_dawn.so``, the WebGPU provider) linked
at load time.

pyke builds ONNX Runtime for x86-64-v3: Intel Haswell, AMD Excavator and
later (2013 on), with AVX2, BMI1/2, FMA, F16C, LZCNT and MOVBE. Its static
initializers already use these: the Einsum operator compiles a regular
expression at load time, with a BMI2 ``mulx`` instruction. On an older
processor the app therefore ended at start with *Illegal instruction*,
before any of its own code ran, and with no message (:issue:`154`, an Ivy
Bridge desktop; reproduced with QEMU emulating Ivy Bridge, and with gdb
stopping in ``_GLOBAL__sub_I_einsum.cc``).

Constraints:

- The app must start on any 64-bit x86 processor; AI is optional.
- pyke ships ONNX Runtime only as a static library.
- The AI features and their speed must stay as they are on supported
  processors.

Decision
--------

On Linux x86-64, ONNX Runtime is a shared library loaded the first time a
model is needed, never when the app starts:

- :repo:`scripts/ort-dylib.sh` links pyke's static build (downloaded by the
  ``picturesque-ort-dylib`` crate, which exists only for that) into
  ``libonnxruntime.so``, exporting only ``OrtGetApiBase``, with Dawn beside
  it (``$ORIGIN`` run path). ``ort`` uses its ``load-dynamic`` feature, so
  no executable links ONNX Runtime or Dawn.
- Before loading it, the app checks the processor for the x86-64-v3
  instructions. Without them, or without the library, AI is unavailable
  with the reason in **Settings → AI acceleration** and the library
  toolbar; AI analyses are not run (photos get no error results) and the
  rest of the app works.
- deb, rpm and AppImage install both libraries in ``lib/picturesque``; the
  release workflow fails if the executable links either at load time.

Windows and Apple Silicon keep the static runtime: every Apple Silicon
processor has the instructions, and Windows was not reported or measured.

Alternatives Considered
-----------------------

Build ONNX Runtime for older processors
   Advantages: AI would work everywhere.

   Disadvantages: a custom ONNX Runtime and Dawn build per release, slower
   on the processors most users have, for hardware that is over ten years
   old. Can come later on top of this decision.

Run AI in a separate process
   Advantages: any crash in the runtime is contained.

   Disadvantages: a second executable, inter-process transport for tensors,
   and more packaging, to solve what a load at first use solves.

Use Microsoft's official shared ONNX Runtime
   Advantages: a supported shared library.

   Disadvantages: no WebGPU provider, a different build per platform, and
   the runtime already measured (ADR-0006, :issue:`42`) would change.

Decision Criteria
-----------------

The app starting everywhere, then keeping AI speed and features, then the
smallest change to builds and packaging.

Rationale
---------

Loading the runtime after a processor check removes the crash without
changing the runtime: the same pyke build, the same providers, the same
results. It is what greycard did for the same problem
(``jessolmstead/greycard#23``).

Consequences
------------

- Linux builds run ``scripts/ort-dylib.sh`` first (``make ort-dylib``; the
  Make targets that need it do so), and packages carry
  ``libonnxruntime.so``.
- Tests, examples and the dev server find the library next to their
  executable or one folder up; ``PICTURESQUE_ORT_DYLIB`` overrides the path.
- ``analysis_status`` gains ``ai_unavailable``.
- Processors without AVX2 have no AI features until a build for them
  exists.
