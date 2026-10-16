# Background Removal changelog

Release history for this standalone IMG.LY-based PhotoCraft plugin. Versions below refer
to this plugin; API and package/worker compatibility markers have separate purposes.

## [1.2.0](https://github.com/brandongunderson/photocraft-background-removal-plugin/releases/tag/v1.2.0) - 2026-10-16

### Changed

- Migrate the panel to the current PhotoCraft Plugin API using the privately included typed
  SDK/API 0.1 and bounded request/response storage.
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

Requires the current panel contract (internal discriminator `2`), package format 2 and
`host.commands.invoke` version 1. The native image protocol remains version 1. The plugin
builds without an editor or sibling source checkout. Model and runtime licenses retain
their separate terms.

See [migration verification](verification/API2-MIGRATION.md) for the release checks and
acceptance limits, and [README.md](README.md) for installation and image limits.
