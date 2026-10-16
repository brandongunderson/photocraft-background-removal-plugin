# Background Removal 1.0.0 verification (2026-10-15)

This report covers the independent plugin and its extracted release package. PhotoCraft host integration, menu/panel snapshots, installed-store restart behavior, and application regression tests are recorded by the host project separately. This is a tested first implementation, not a claim of exhaustive compatibility or pixel-perfect segmentation.

## Package tested

- File: `PhotoCraft-Background-Removal.pcplugin`
- Size: **281,780,939 bytes**
- SHA256: `5f1e30513cffa5bae19c32a22c90d245ca91f93a3471e77b9fb7988e1f943d90`
- Panel ID/version: `org.photocraft.background-removal` / `1.0.0`
- CPU worker built in release mode, Windows x64 GNU Rust toolchain.
- ONNX Runtime 1.22.0, Rust `ort` 2.0.0-rc.10, dynamically loaded from the package.
- All listed runtime/model/license assets passed SHA256 checks during packaging. ZIP CRC verification passed on extraction.

## Automated checks

**Six native processing/protocol tests passed:** exact RGB NCHW normalization; full-size foreground/background/mask composition with original alpha; probability handling without a second sigmoid; malformed tensor shape/non-finite output rejection; source dimension bounds and cancellation; 16-bit RGBA/grayscale decoding; strict protocol and path/model/output guards.

**Three guest tests passed:** the panel requests only `image.removeBackground`; all three model choices and three result choices map to valid typed options; default model agrees with the manifest; missing document or unknown event never requests inference.

Both projects passed formatting and `cargo clippy --locked --offline --all-targets -- -D warnings`. The environment emitted incremental-cache hard-link fallback warnings; there were no Clippy diagnostics. Both native and WASM release builds succeeded.

## Actual packaged engine checks

The tests extracted the full `.pcplugin` and launched its packaged release worker with the packaged ONNX Runtime and models. No external companion or runtime path was selected. Every successful result had the input dimensions, 8-bit RGBA pixels, and an embedded sRGB ICC profile. Existing transparent/semitransparent source pixels remained protected, masks were grayscale and opaque, and the source file was unchanged.

| Case | Result | Elapsed on this development machine |
|---|---|---:|
| Full-precision `isnet`, 96 × 64 RGBA | Passed | 1.047 s |
| Half-precision `isnet_fp16`, 96 × 64 RGBA | Passed | 1.282 s |
| Quantized `isnet_quint8`, 96 × 64 RGBA | Passed | 1.125 s |
| Keep background, FP16 | Passed | 1.203 s |
| Subject mask, FP16 | Passed | 1.187 s |
| Cancel by stdin byte after inference began | Exited with error; no output | 0.047 s after cancellation |
| Cancel by stdin EOF after inference began | Exited with error; no output | 0.062 s after cancellation |
| 24 MP synthetic image, 6000 × 4000, FP16 | Full-size result passed | 1.734 s |
| NASA public-domain photo, 1280 × 1249, FP16 | Full-size result passed | 1.312 s |

Timings include process startup, model loading, decoding, processing, and PNG output. The 24 MP input is a simple generated image, not a representative image-quality benchmark. These measurements do not predict other hardware or image types. Machine-readable results are in `package-smoke.json`; per-job progress/error logs accompany it. Reproduce with `scripts/smoke-package.py` (requires Python and Pillow).

An invalid-runtime failure check also passed: a broken test DLL produces an actionable error naming the Windows/Visual C++ prerequisites and official redistributable link, exits with an error, and creates no output. The test used a separate temporary fixture, without modifying the installed test package.

The NASA photograph is for local visual verification, not included in the installable package. Its source/license provenance is recorded in `photo-source.json`; transformed results are plugin outputs, not NASA originals.

## Remaining limits

- Wider Windows/CPU configurations and missing-system-runtime scenarios have not been exhaustively tested.
- Segmentation quality is model-dependent. Hair, glass, translucent materials, fine details, unusual subjects, and difficult backgrounds require manual evaluation/refinement.
- The AI's internal mask resolution is 1024 × 1024. Returning original dimensions does not add model detail beyond that resolution.
- Worker input bounds and host memory estimates reduce excessive allocations; they are not a guarantee that every permitted image fits available RAM.
- CPU inference is implemented. No GPU execution-provider acceleration is claimed.
