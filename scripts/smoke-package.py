"""Exercise the extracted installable package, including real ORT inference."""
from pathlib import Path
import ctypes
from ctypes import wintypes
import hashlib
import json
import os
import queue
import subprocess
import threading
import time
import zipfile
from PIL import Image, ImageDraw

root = Path(__file__).resolve().parents[1]
verify = root / "verification"
verify.mkdir(exist_ok=True)
package = root / "dist/PhotoCraft-Background-Removal.pcplugin"
staging = root / "dist/smoke-installed"
staging.mkdir(parents=True, exist_ok=True)
with zipfile.ZipFile(package) as archive:
    assert archive.testzip() is None
    metadata = json.loads(archive.read("package.json"))
    assert metadata["packageVersion"] == 2
    assert metadata["native"]["service"] == "plugin.processImage"
    archive.extractall(staging)
worker = staging / "photocraft-background-worker.exe"
source = verify / "input.png"
image = Image.new("RGBA", (96, 64), (240, 240, 240, 255))
draw = ImageDraw.Draw(image)
draw.ellipse((25, 5, 70, 60), fill=(180, 50, 30, 255))
draw.rectangle((0, 0, 7, 63), fill=(20, 70, 130, 0))
draw.rectangle((88, 0, 95, 63), fill=(20, 70, 130, 128))
image.save(source)
source_hash = hashlib.sha256(source.read_bytes()).hexdigest()
results = []

def local_vc_modules(pid):
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    kernel.OpenProcess.restype = wintypes.HANDLE
    kernel.CloseHandle.argtypes = [wintypes.HANDLE]
    kernel.K32EnumProcessModulesEx.argtypes = [wintypes.HANDLE, ctypes.POINTER(wintypes.HMODULE), wintypes.DWORD, ctypes.POINTER(wintypes.DWORD), wintypes.DWORD]
    kernel.K32GetModuleFileNameExW.argtypes = [wintypes.HANDLE, wintypes.HMODULE, wintypes.LPWSTR, wintypes.DWORD]
    process = kernel.OpenProcess(0x410, False, pid)
    assert process, ctypes.WinError(ctypes.get_last_error())
    try:
        modules = (wintypes.HMODULE * 1024)()
        needed = wintypes.DWORD()
        assert kernel.K32EnumProcessModulesEx(process, modules, ctypes.sizeof(modules), ctypes.byref(needed), 3)
        assert needed.value <= ctypes.sizeof(modules)
        found = {}
        for module in modules[:needed.value // ctypes.sizeof(wintypes.HMODULE)]:
            filename = ctypes.create_unicode_buffer(32768)
            assert kernel.K32GetModuleFileNameExW(process, module, filename, len(filename))
            path = Path(filename.value)
            if path.name.lower() in {"msvcp140.dll", "msvcp140_1.dll", "vcruntime140.dll", "vcruntime140_1.dll"}:
                assert path.parent.resolve() == staging.resolve(), path
                found[path.name.lower()] = str(path)
        assert len(found) == 4, found
        return found
    finally:
        kernel.CloseHandle(process)


def run(name, model, output="foreground", source=source, cancel=None, timeout=240):
    destination = verify / (name + ".png")
    destination.unlink(missing_ok=True)
    job = verify / (name + "-job.json")
    with Image.open(source) as original:
        width, height = original.size
    job.write_text(json.dumps({"apiVersion": 1, "input": str(source), "output": str(destination), "options": {"model": model, "output": output}, "outputSize": {"width": width, "height": height}, "colorSpace": "srgb"}), encoding="utf-8")
    started = time.monotonic()
    process = subprocess.Popen([str(worker), "--job", str(job)], stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, creationflags=subprocess.CREATE_NO_WINDOW)
    lines = queue.Queue()
    def reader():
        for line in process.stderr:
            lines.put(line.decode("utf-8", errors="replace"))
        lines.put(None)
    threading.Thread(target=reader, daemon=True).start()
    logs = []
    vc_modules = None
    cancelled_at = None
    try:
        while True:
            if time.monotonic() - started > timeout:
                raise TimeoutError(name)
            try:
                line = lines.get(timeout=.25)
            except queue.Empty:
                continue
            if line is None:
                break
            logs.append(line)
            try:
                progress = json.loads(line)
            except json.JSONDecodeError:
                continue
            if progress.get("progress") == 30:
                vc_modules = local_vc_modules(process.pid)
            if cancel and progress.get("progress") == 30 and cancelled_at is None:
                time.sleep(.25)
                cancelled_at = time.monotonic()
                if cancel == "byte":
                    process.stdin.write(b"cancel\n")
                    process.stdin.flush()
                else:
                    process.stdin.close()
        code = process.wait(timeout=10)
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()
        if not process.stdin.closed:
            process.stdin.close()
        process.stderr.close()
    (verify / (name + ".log")).write_text("".join(logs), encoding="utf-8")
    elapsed = round(time.monotonic() - started, 3)
    item = {"test": name, "model": model, "output": output, "seconds": elapsed, "exitCode": code}
    assert vc_modules is not None, logs
    item["appLocalVcModules"] = vc_modules
    if cancel:
        assert cancelled_at is not None, logs
        item["cancelExitSeconds"] = round(time.monotonic() - cancelled_at, 3)
        assert code != 0 and not destination.exists(), logs
        assert item["cancelExitSeconds"] < 10, item
    else:
        assert code == 0, logs
        with Image.open(source) as original, Image.open(destination) as result:
            assert result.size == original.size and result.mode == "RGBA"
            assert result.info.get("icc_profile")
            item["dimensions"] = list(result.size)
            if source.name == "input.png" and output != "mask":
                assert result.getpixel((0, 0))[3] == 0
                assert result.getpixel((90, 0))[3] <= 128
            if output == "mask":
                assert result.getextrema()[3] == (255, 255)
                assert result.getpixel((30, 30))[0] == result.getpixel((30, 30))[1] == result.getpixel((30, 30))[2]
    results.append(item)
    print(json.dumps(item), flush=True)

def reject_invalid_jobs():
    destination = verify / "invalid-output.png"
    valid = {"apiVersion": 1, "input": str(source), "output": str(destination), "options": {"model": "isnet_fp16", "output": "foreground"}, "outputSize": {"width": 96, "height": 64}, "colorSpace": "srgb"}
    mutations = [
        ("unsupported-api", "apiVersion", 2),
        ("unsupported-color", "colorSpace", "display-p3"),
        ("resize-rejected", "outputSize", {"width": 192, "height": 128}),
        ("unknown-model", "options", {"model": "../isnet", "output": "foreground"}),
        ("unknown-option", "options", {"model": "isnet", "output": "foreground", "unknown": True}),
    ]
    for name, key, value in mutations:
        job = verify / (name + "-job.json")
        request = dict(valid)
        request[key] = value
        job.write_text(json.dumps(request), encoding="utf-8")
        process = subprocess.run([str(worker), "--job", str(job)], input=b"", capture_output=True, timeout=10, creationflags=subprocess.CREATE_NO_WINDOW)
        assert process.returncode != 0 and not destination.exists(), name
        assert b'"progress"' not in process.stderr, process.stderr
        results.append({"test": name, "exitCode": process.returncode, "rejectedBeforeInference": True})


reject_invalid_jobs()
for model in ["isnet_fp16", "isnet", "isnet_quint8"]:
    run("model-" + model, model)
run("background", "isnet_fp16", output="background")
run("mask", "isnet_fp16", output="mask")
run("cancel-byte", "isnet_fp16", cancel="byte")
run("cancel-eof", "isnet_fp16", cancel="eof")
large = verify / "input-24mp.png"
large_image = Image.new("RGBA", (6000, 4000), (224, 224, 224, 255))
ImageDraw.Draw(large_image).ellipse((2000, 400, 4000, 3600), fill=(140, 35, 25))
large_image.save(large)
large_image.close()
run("benchmark-24mp", "isnet_fp16", source=large)
photo = verify / "nasa-source.jpg"
if photo.exists():
    photo_png = verify / "nasa-source.png"
    with Image.open(photo) as original:
        original.convert("RGBA").save(photo_png)
    run("nasa-result", "isnet_fp16", source=photo_png)
assert hashlib.sha256(source.read_bytes()).hexdigest() == source_hash
report = {"package": package.name, "sha256": hashlib.sha256(package.read_bytes()).hexdigest(), "bytes": package.stat().st_size, "sourceUnchanged": True, "tests": results}
(verify / "package-smoke.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
