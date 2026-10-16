# PhotoCraft Background Removal

A PhotoCraft plugin based on [dnanhkhoa/rust-background-removal](https://github.com/dnanhkhoa/rust-background-removal), using IMG.LY's IS-Net models. Open **Plugins → Background Removal** for the controls and comparison preview. A bundled worker performs local CPU inference; PhotoCraft manages its lifetime and cancellation.

[Source repository](https://github.com/brandongunderson/photocraft-background-removal-plugin) · [Changelog](CHANGELOG.md)

[Download the latest plugin](https://github.com/brandongunderson/photocraft-background-removal-plugin/releases/latest/download/PhotoCraft-Background-Removal.pcplugin) · [All releases](https://github.com/brandongunderson/photocraft-background-removal-plugin/releases)

## Install and use

1. Run a PhotoCraft build supporting panel API 2, package format 2 and `host.commands.invoke` version 1, with the general `plugin.processImage` API.
2. Choose **Plugins → Manage Plugins → Browse** and select `PhotoCraft-Background-Removal.pcplugin`.
3. Review the native-code permission and select **Install plugin** or **Update plugin**. The current manager chooses the action from the installed and selected versions.
4. Open an image and choose **Plugins → Background Removal**.
5. Choose a model and result, then **Create result**. Compare the preview with the divider, then **Apply** or **Discard**. Apply can open a new unsaved tab or add a layer at the original dimensions. The source stays unchanged until Apply; adding a layer is undoable.

Requirements: Windows x64, Windows 10 build 19041 or later or Windows 11. This package includes its models, ONNX Runtime, and the Microsoft VC runtime DLLs imported by ONNX; no separate VC runtime installation is required. Windows itself supplies its normal system libraries and Universal CRT. It makes no model downloads during use. CPU inference does not need a discrete GPU. Native inference can use substantial RAM and take several seconds or longer; available memory and image size affect performance.

## Controls

| Model | ID | Weight storage |
|---|---|---|
| Standard · full precision | `isnet` | Float32, about 176 MB |
| Compact · half precision | `isnet_fp16` | Float16, about 88 MB |
| Small · quantized | `isnet_quint8` | Quantized, about 44 MB |

These describe weight formats, not guaranteed performance rankings. CPU runtimes may convert or execute half-precision/quantized weights differently. The compact half-precision model is the default. Compare results on your own photographs.

| Result | Behavior |
|---|---|
| Keep subject | Original RGB with subject-mask alpha multiplied by existing alpha |
| Keep background | Original RGB with inverse subject-mask alpha multiplied by existing alpha; does not reconstruct content hidden behind the subject |
| Subject mask | Opaque grayscale mask, white for retained foreground; includes existing source transparency |

The plugin processes the visible flattened composite. PhotoCraft converts supported 8-bit and 16-bit RGB/grayscale source documents through its color management to **8-bit RGBA sRGB** before invoking the worker. Results are also 8-bit RGBA sRGB; PhotoCraft retains document DPI. A new result tab contains flattened pixels. The worker accepts at most 32,768 pixels per side and 64 megapixels; PhotoCraft's generic processing and working-memory limits can reject inputs below that limit, and available system memory may impose lower practical limits. The AI operates at 1024 × 1024 internally and restores the mask to the original dimensions; original source RGB pixels are not resized. Fine hair, translucency, difficult backgrounds, and small details may need manual refinement.

## Source layout

```text
authoring.json               Panel API 2 descriptor and package format 2 metadata
extension/                   Typed Rust WASM panel and private SDK
src/worker.rs                One-job native executable, progress and cancellation
src/engine.rs                Input color conversion, IS-Net inference and mask composition
src/protocol.rs              Strict bounded JSON protocol
tests/processing.rs          Pixel, protocol and bounds tests
scripts/build-plugin.ps1     Builds worker + guest and creates .pcplugin
scripts/fetch-assets.ps1     Retrieves pinned binary/model assets
scripts/fetch-vc-runtime.ps1  Extracts app-local VC DLLs without running an installer
runtime/downloads.json       Exact asset URLs, SHA256 values and upstream revisions
runtime/vc-runtime.json      Pinned official VC runtime archive and per-file hashes
runtime/models/              Three IMG.LY ONNX models
runtime/bin/                 ONNX Runtime Windows x64 CPU binaries
runtime/licenses/            Runtime/model licenses and upstream notices
vendor/rust-background-removal/  Unmodified upstream reference source and provenance
licenses/                    Bundled Rust dependency license texts
```

This project has its own Cargo workspace and does not depend on PhotoCraft editor source or the former upscale companion. Model ids, options, inference, dimension restrictions, dependency assets, and required asset checks belong to this plugin. PhotoCraft supplies only the general installer, permission gate, image transport, progress/cancellation, and preview/apply transaction. Updating models or settings requires rebuilding this plugin, without changing the editor. The upstream CLI's preprocessing informed `src/engine.rs`; its CLI and HTTP server are not compiled or packaged.

## Build

Install Rust 1.95 or later with the `wasm32-unknown-unknown` target and a working Windows x64 linker, plus Python 3.11 or later for collecting dependency licenses. Downloaded inference binaries and model files are excluded from Git. Restore the exact runtime assets with `scripts/fetch-assets.ps1`, which uses the URLs and SHA256 hashes in `runtime/downloads.json`. Run from this project:

```powershell
rustup target add wasm32-unknown-unknown
./scripts/fetch-assets.ps1
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --manifest-path extension/Cargo.toml
./scripts/build-plugin.ps1 -Python python
```

The output is `dist/PhotoCraft-Background-Removal.pcplugin`. Add `-Offline` when all Rust dependencies and assets are cached. `-DebugWorker` builds a debug native worker for development. The script validates every listed runtime asset's SHA256, builds native and WASM binaries, checks the worker is Windows x64, and creates a ZIP with files directly at its root. Install the complete package, not the `.pcext` alone.

Version 1.2.0 uses `packageVersion: 2`, the general service `plugin.processImage`, entry `photocraft-background-worker.exe`, and a `plugin.pcext` module. The panel grants only `plugin.processImage` and document summary access. The host binds processing to this package's worker; the panel cannot select another plugin or executable. Older hosts that only support the former typed AI services cannot install this release. Update the editor first and replace the installed package; the stable plugin id is unchanged and the new generic processing permission must be approved.

## Repository contents

The native worker and `extension/` panel have separate Cargo workspaces and lockfiles. Both build independently of the PhotoCraft editor and other plugins. The included `vendor/rust-background-removal/` directory preserves reference source and attribution.

Source history includes lockfiles, manifests, build scripts, vendored source, licenses and asset provenance. Restore downloaded binaries and model weights from `runtime/downloads.json`. Cargo caches, generated packages, test images and machine-specific validation reports stay local. The Markdown summaries in `verification/` record the tested releases and limits; their referenced JSON reports accompany local test runs.

Distribute the complete `.pcplugin` through GitHub Releases. Keep its stable plugin id when publishing updates, and increase the plugin version for a new release. Asset restoration is a developer build step; the installed plugin includes all three models.

## Worker contract

PhotoCraft writes a private request JSON and runs `photocraft-background-worker.exe --job <absolute-request-path>` with a piped stdin lifetime. No shell, server, or command string is involved.

```json
{
  "apiVersion": 1,
  "input": "C:\\temporary\\input.png",
  "output": "C:\\temporary\\output.png",
  "options": {"model": "isnet", "output": "foreground"},
  "outputSize": {"width": 1920, "height": 1080},
  "colorSpace": "srgb"
}
```

Requests are limited to 16 KiB, unknown fields are rejected, API version must be 1, colorSpace must be `srgb`, input/output must be distinct absolute paths, and an existing output is never overwritten. The input header must be 8-bit RGBA PNG, and `outputSize` must exactly match the source dimensions; background removal does not resize. Models are selected from the three fixed IDs and result mode is foreground/background/mask. The panel sends these as `options` with processor `native`, preview enabled, and no width/height override. The worker loads runtime assets relative to its installed executable. It emits bounded JSON progress on stderr. `cancel\n`, any other stdin byte, or EOF requests cancellation; active inference uses ONNX Runtime's termination flag, and the host can terminate and reap the process. Output is encoded into a temporary file, then published only after success and a final cancellation check. PhotoCraft validates the output and source-document identity before accepting a preview or applying a result.

## Model versions and licensing

Assets were checked on **2026-10-15**. Full and half-precision weights are pinned to IMG.LY's `isnet-general-onnx` revision `440dea96dd4a3b06bbbf5abec3e26569dd7ec49f`. The quantized variant is pinned to IMG.LY's background-removal repository revision `12f56cc4f2a90d624e165a715748d22efc7a1d93`. See `runtime/downloads.json` for exact sources/hashes. These are the public upstream assets verified on that date, not an automatic “latest” download during installation or use.

ONNX Runtime is pinned to 1.22.0 with Rust `ort` 2.0.0-rc.10 for compatible native APIs. This code uses the MIT Rust implementation and separately supplied model assets; it does not compile or embed IMG.LY's JavaScript SDK. See [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md), `runtime/licenses`, and `vendor/rust-background-removal/LICENSE` for provenance and licensing.

Microsoft VC runtime 14.44.35211.0 is extracted as data from the official pinned x64 redistributable; its installer is never run. `msvcp140.dll`, `msvcp140_1.dll`, `vcruntime140.dll`, and `vcruntime140_1.dll` are installed beside the worker, not into PhotoCraft or Windows. The original runtime license is included at `runtime/licenses/MICROSOFT-VC-RUNTIME-LICENSE.rtf`; [the runtime provenance note](runtime/licenses/MICROSOFT-VC-RUNTIME-NOTICE.md) records redistribution conditions. A publisher must independently have the qualifying Visual Studio license and distribution rights; this local build does not establish that entitlement. These proprietary files are not covered by this project's MIT license. App-local runtime security updates require a new plugin release.

## Limits and troubleshooting

- **Cannot load bundled AI engine:** verify the supported Windows version and reinstall the complete plugin to restore its bundled ONNX and VC DLLs.
- **Unsupported package or command:** run a PhotoCraft build with package version 2 and `plugin.processImage`, install the full `.pcplugin`, then open the installed panel.
- **Slow or memory-heavy processing:** close other large documents/applications or use a smaller input. Smaller model storage does not guarantee faster CPU execution.
- **Inaccurate edges:** compare models and refine the resulting transparency or mask manually. This segmentation model is not a guarantee of pixel-perfect hair or glass extraction.
- **Update/remove blocked during processing:** cancel or wait for completion before replacing/removing the package.

The native worker runs with your OS account's permissions. The WASM panel is sandboxed; the worker is not covered by that sandbox. Install packages from publishers you trust.

## API 2 compatibility (1.2.0)

This release requires a PhotoCraft host supporting **panel API 2**, package format 2 and
the `host.commands.invoke` capability at version 1. API 1 panels are no longer supported.
The stable plugin id, model choices, processing algorithms and worker protocols are preserved.
The native image/task protocol remains version 1; it is separate from the panel API.

Edit `authoring.json` for both the panel descriptor and archive metadata. The panel's
build script validates it with the private SDK and generates the embedded descriptor and
bindings into Cargo output. The package builder derives its root `package.json` from the
same source. `extension/vendor/` contains this project's SDK/API sources and their
`SDK-RELEASE.json` hashes. Builds require no PhotoCraft source checkout or sibling plugin.
The SDK handles bounded ABI buffers and API 2 event validation, including additive host
metadata. Failed/unknown/observer events propose no processing and preserve panel state.
