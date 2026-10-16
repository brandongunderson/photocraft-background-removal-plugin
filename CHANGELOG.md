# Background Removal changelog

Release history for this standalone IMG.LY-based PhotoCraft plugin. Versions below refer
to this plugin; API and package/worker compatibility markers have separate purposes.

## [1.2.1](https://github.com/brandongunderson/photocraft-background-removal-plugin/releases/tag/v1.2.1) - 2026-10-16

### Added

- Opt into PhotoCraft’s explicit GitHub release checks using this plugin’s own repository and
  exact package asset. Updates require review and confirmation.

### Changed

- Refresh the privately bundled PhotoCraft Plugin API and typed SDK sources.
- Preserve the stable plugin identity, controls, model choices and native processing protocols.
- Update independent-build and compatibility documentation.

### Validation

The final descriptor passed 4 panel tests, formatting and all-target Clippy. The unchanged
native worker previously passed 7 tests and its formatting/Clippy checks. Final release
packaging, exact GitHub update metadata and current-host conformance passed. Real native
processing, preview, Apply/Discard, Undo/Redo, stale-source rejection, cancellation and removal
passed on the final package in isolated stores. See [API compatibility](verification/API-COMPATIBILITY.md)
for the package hash and limits.

## [1.2.0](https://github.com/brandongunderson/photocraft-background-removal-plugin/releases/tag/v1.2.0) - 2026-10-16

### Changed

- Use privately included typed SDK/API sources and bounded request/response storage.
- Generate the embedded descriptor, bindings and package metadata from `authoring.json`.
- Declare `host.commands.invoke`, preserve the stable plugin ID and model choices, and
  prevent observer or unknown events from starting inference.

### Processing retained in this release

- Local CPU background removal with three bundled IMG.LY IS-Net weight variants.
- Keep-subject, keep-background and subject-mask output, with source transparency retained
  in the effective mask.
- Progress/cancellation, comparison preview and Apply through the shared
  `plugin.processImage` interface.
- A Windows x64 package containing ONNX Runtime, its app-local Microsoft runtime dependencies,
  pinned models and license notices. No discrete GPU is required.

### Compatibility

Uses package format 2 and
`host.commands.invoke` version 1. The native image protocol remains version 1. The plugin
builds without an editor or sibling source checkout. Model and runtime licenses retain
their separate terms.

See [API compatibility](verification/API-COMPATIBILITY.md) for the release checks and
acceptance limits, and [README.md](README.md) for installation and image limits.
