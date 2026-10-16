# Third-party notices

## Original Rust implementation

[dnanhkhoa/rust-background-removal](https://github.com/dnanhkhoa/rust-background-removal), commit `9488f5b0a0a0d800c57f1af83a780dabbfb3780a`, copyright (c) 2023 Khoa Duong, MIT. The original project also credits David Horner. Its RGB NCHW preprocessing and IS-Net inference approach were adapted into `src/engine.rs`; the original CLI and server are not included in the executable. Unmodified upstream reference files and provenance are retained under `vendor/rust-background-removal`. Its full MIT license accompanies the installed package as `licenses/rust-background-removal-MIT.txt`.

## IMG.LY IS-Net weights

Full and half precision: [imgly/isnet-general-onnx](https://huggingface.co/imgly/isnet-general-onnx), revision `440dea96dd4a3b06bbbf5abec3e26569dd7ec49f`. The published model card identifies the license as MIT. Quantized model: IMG.LY [background-removal-js](https://github.com/imgly/background-removal-js), revision `12f56cc4f2a90d624e165a715748d22efc7a1d93`, `bundle/models/isnet_quint8`; that project's third-party notices identify its IS-Net model under MIT. Copies of the model card and notices are included in `runtime/licenses`.

The IS-Net architecture is associated with [Highly Accurate Dichotomous Image Segmentation](https://github.com/xuebinqin/DIS). Its Apache-2.0 license is preserved as `runtime/licenses/DIS-LICENSE.md` for provenance. This package does not include the IMG.LY JavaScript SDK. Model licenses and the SDK's license are separate.

## ONNX Runtime

[Microsoft ONNX Runtime](https://github.com/microsoft/onnxruntime), Windows x64 CPU version 1.22.0, MIT. Full license and third-party notices accompany the runtime in `runtime/licenses/onnxruntime-LICENSE` and `onnxruntime-ThirdPartyNotices.txt`.

ONNX Runtime requires Windows system components and Microsoft VC runtime libraries. The plugin bundles the unmodified x64 `msvcp140.dll`, `msvcp140_1.dll`, `vcruntime140.dll`, and `vcruntime140_1.dll` version 14.44.35211.0 beside its worker. These are extracted from Microsoft's pinned official redistributable, never copied from Windows system directories. Their original end-user license and provenance/redistribution conditions accompany them at `runtime/licenses/MICROSOFT-VC-RUNTIME-LICENSE.rtf` and `MICROSOFT-VC-RUNTIME-NOTICE.md`. The publisher must have qualifying Visual Studio redistribution rights before distributing them. These proprietary DLLs are not MIT-licensed. Windows supplies its own normal system libraries and Universal CRT.

## Rust dependencies

The executable uses `ort` 2.0.0-rc.10 (MIT/Apache-2.0), `image` (MIT/Apache-2.0), `moxcms` (BSD-3-Clause or Apache-2.0), `serde`, `serde_json`, `tempfile`, and their transitive dependencies. Exact versions are recorded in the two Cargo.lock files. Full locally published dependency license texts are collected in `licenses/rust-dependencies.txt`. No dependency on PhotoCraft source or the former upscale companion is included.

Binary and model source URLs, exact revisions, sizes, and SHA256 digests are recorded in `runtime/downloads.json` and `runtime/vc-runtime.json`.
