"""Collect published Rust dependency license texts for binary redistribution."""
from pathlib import Path
import json
import subprocess

root = Path(__file__).resolve().parents[1]
packages = {}
for manifest, target in [(root / "Cargo.toml", "x86_64-pc-windows-gnu"), (root / "extension/Cargo.toml", "wasm32-unknown-unknown")]:
    metadata = json.loads(subprocess.check_output([
        "cargo", "metadata", "--locked", "--offline", "--format-version", "1",
        "--manifest-path", str(manifest), "--filter-platform", target
    ], text=True))
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    included = set()
    remaining = [metadata["resolve"]["root"]]
    while remaining:
        current = remaining.pop()
        if current not in included:
            included.add(current)
            remaining.extend(nodes[current]["dependencies"])
    for package in metadata["packages"]:
        if package.get("source") and package["id"] in included:
            packages[(package["name"], package["version"])] = package

parts = ["Published Rust dependency licenses\nGenerated from the pinned Cargo.lock files.\n"]
missing = []
for key, package in sorted(packages.items()):
    folder = Path(package["manifest_path"]).parent
    files = sorted(p for p in folder.rglob("*") if p.is_file()
                   and p.name.lower().startswith(("license", "licence", "copying", "copyright")))
    parts.append(f"\n{'=' * 72}\n{key[0]} {key[1]}\nLicense: {package.get('license')}\nSource: {package.get('repository')}\n")
    if not files:
        missing.append(key)
    for path in files:
        parts.append(f"\n--- {path.relative_to(folder).as_posix()} ---\n{path.read_text(encoding='utf-8', errors='replace')}\n")
if missing:
    raise RuntimeError(f"Missing published license files: {missing}")
(root / "licenses").mkdir(exist_ok=True)
(root / "licenses/rust-dependencies.txt").write_text("\n".join(parts), encoding="utf-8")
print(f"Collected licenses for {len(packages)} pinned dependencies.")
