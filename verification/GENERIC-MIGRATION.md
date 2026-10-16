# Background Removal generic API migration

Machine-specific JSON reports referenced below remain with local validation runs.
This source repository retains the release summary and the scripts used to run its checks.

Version 1.1.0 uses package version 2 and `plugin.processImage`. Its stable id is
`org.photocraft.background-removal`. Native options, model allowlist, output modes,
input/output validation, dependency assets and inference live in this independent
plugin project; neither Cargo workspace depends on PhotoCraft source.

Artifact: `dist/PhotoCraft-Background-Removal.pcplugin`

- SHA256: `faf9dfae43d7f7a55e7193be93de354d1a31211ef06ea8aa4de4f688a662bc05`
- Compressed bytes: 282090807
- Archive files: 26
- Native processing tests: 7 passed.
- Panel contract tests: 3 passed.
- Native and panel all-target Clippy with warnings denied: passed.
- Native and panel Rust formatting: passed.
- Locked offline release worker and WASM panel builds: passed.
- Pinned model, ONNX Runtime and VC runtime asset verification/restoration offline: passed.
- Fresh VC runtime restoration from only the fetch script, manifest and cached official
  archive: passed offline with GNU tar first on the developer PATH. The script uses
  `Windows/System32/tar.exe` explicitly for CAB support. All four DLLs and the original
  license matched pinned hashes, temporary cabinets were cleaned, and existing plugin
  runtime files were unchanged. Evidence: `vc-runtime-restoration.json`.
- Extracted exact-package smoke: 14 passed (5 invalid requests rejected before inference;
  all 3 models; subject/background/mask output; cancel and EOF; 24-megapixel image;
  NASA photograph). Original synthetic source hash retained.
- Active cancellation exited in 0.110 / 0.109 seconds with no output.
- 24-megapixel smoke completed in 1.781 seconds on this development machine.
- PE dependency closure and live worker module enumeration verified all four Microsoft
  VC libraries load from the extracted plugin directory. Normal Windows OS libraries
  remain supplied by Windows; no system VC runtime installer was run.

Machine-readable evidence: `generic-migration-artifacts.json` (package and imports)
and `package-smoke.json` (actual results, output sizes and loaded runtime paths).
Core install/preview/Apply/restart integration is covered separately by the host project tests.
These plugin-only checks do not certify the editor UI or other operating systems.

The Microsoft VC runtime DLLs are proprietary components and require qualifying Visual
Studio redistribution rights before publication; this local package does not establish
publisher entitlement. Exact official source, pinned hashes and original end-user
license are in `runtime/vc-runtime.json` and `runtime/licenses/MICROSOFT-VC-RUNTIME-*`.
The installed plugin requires supported Windows x64, not PhotoCraft-specific AI adapters.
