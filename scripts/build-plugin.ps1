param([switch]$DebugWorker, [switch]$Offline, [string]$Python = 'python')
$ErrorActionPreference = 'Stop'
$pluginRoot = [System.IO.Path]::GetFullPath((Split-Path $PSScriptRoot))
$pluginDist = Join-Path $pluginRoot 'dist'
$pluginStage = Join-Path $pluginDist ('stage-' + [Guid]::NewGuid().ToString('N'))
$pluginArchive = Join-Path $pluginDist 'PhotoCraft-Background-Removal.pcplugin'
$pluginTemporary = Join-Path $pluginDist ('package-' + [Guid]::NewGuid().ToString('N') + '.tmp')
$pluginProfile = if ($DebugWorker) { 'debug' } else { 'release' }

Push-Location $pluginRoot
try {
    # Validate every required binary and model against the pinned source manifest before shipping.
    $pluginDownloads = Get-Content -LiteralPath (Join-Path $pluginRoot 'runtime/downloads.json') -Raw | ConvertFrom-Json
    foreach ($pluginAsset in $pluginDownloads.files) {
        $pluginAssetPath = Join-Path $pluginRoot ('runtime/' + $pluginAsset.path)
        if (-not (Test-Path -LiteralPath $pluginAssetPath -PathType Leaf)) {
            throw "Missing bundled runtime asset: $($pluginAsset.path)"
        }
        $pluginHash = (Get-FileHash -LiteralPath $pluginAssetPath -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($pluginHash -ne $pluginAsset.sha256) { throw "Runtime checksum mismatch: $($pluginAsset.path)" }
    }
    & (Join-Path $PSScriptRoot 'fetch-vc-runtime.ps1') -Offline
    if (-not [Environment]::Is64BitOperatingSystem -or $env:OS -ne 'Windows_NT') {
        throw 'This package currently targets 64-bit Windows.'
    }
    $pluginArgs = @('build', '--locked', '--bin', 'photocraft-background-worker', '--target-dir', 'target')
    if (-not $DebugWorker) { $pluginArgs += '--release' }
    if ($Offline) { $pluginArgs += '--offline' }
    & cargo @pluginArgs
    if ($LASTEXITCODE -ne 0) { throw 'Plugin processing engine build failed.' }
    Push-Location (Join-Path $pluginRoot 'extension')
    try {
        $pluginArgs = @('build', '--locked', '--release', '--target', 'wasm32-unknown-unknown', '--target-dir', 'target')
        if ($Offline) { $pluginArgs += '--offline' }
        & cargo @pluginArgs
        if ($LASTEXITCODE -ne 0) { throw 'Plugin module build failed. Install the wasm32-unknown-unknown Rust target.' }
    } finally { Pop-Location }

    $pluginWorkerPath = Join-Path $pluginRoot "target/$pluginProfile/photocraft-background-worker.exe"
    $pluginWorkerBytes = [System.IO.File]::ReadAllBytes($pluginWorkerPath)
    if ($pluginWorkerBytes.Length -lt 64 -or $pluginWorkerBytes[0] -ne 77 -or $pluginWorkerBytes[1] -ne 90) {
        throw 'The built processing engine is not a Windows executable.'
    }
    $pluginPeOffset = [BitConverter]::ToInt32($pluginWorkerBytes, 60)
    if ($pluginPeOffset -lt 0 -or $pluginPeOffset -gt $pluginWorkerBytes.Length - 6 -or
        [BitConverter]::ToUInt32($pluginWorkerBytes, $pluginPeOffset) -ne 17744 -or
        [BitConverter]::ToUInt16($pluginWorkerBytes, $pluginPeOffset + 4) -ne 34404) {
        throw 'The built processing engine must target Windows x86_64.'
    }
    & $Python (Join-Path $PSScriptRoot 'collect-licenses.py')
    if ($LASTEXITCODE -ne 0) { throw 'Dependency license collection failed.' }
    New-Item -ItemType Directory -Path $pluginStage -Force | Out-Null
    # The same authoring source generates the embedded panel descriptor and archive metadata.
    $pluginAuthoring = Get-Content -LiteralPath (Join-Path $pluginRoot 'authoring.json') -Raw | ConvertFrom-Json
    $pluginPackage = $pluginAuthoring.package | ConvertTo-Json -Depth 32
    [IO.File]::WriteAllText((Join-Path $pluginStage 'package.json'), $pluginPackage, [Text.UTF8Encoding]::new($false))
    New-Item -ItemType Directory -Path (Join-Path $pluginStage 'licenses/photocraft-sdk') -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $pluginRoot 'extension/vendor/plugin-sdk/LICENSE-MIT') -Destination (Join-Path $pluginStage 'licenses/photocraft-sdk/LICENSE-MIT')
    Copy-Item -LiteralPath (Join-Path $pluginRoot 'extension/vendor/plugin-sdk/LICENSE-APACHE') -Destination (Join-Path $pluginStage 'licenses/photocraft-sdk/LICENSE-APACHE')
    Copy-Item -LiteralPath (Join-Path $pluginRoot 'extension/vendor/SDK-RELEASE.json') -Destination (Join-Path $pluginStage 'licenses/photocraft-sdk/SDK-RELEASE.json')
    foreach ($pluginName in @('LICENSE', 'THIRD-PARTY-NOTICES.md', 'README.md')) {
        Copy-Item -LiteralPath (Join-Path $pluginRoot $pluginName) -Destination $pluginStage
    }
    Copy-Item -LiteralPath $pluginWorkerPath -Destination $pluginStage
    Copy-Item -LiteralPath (Join-Path $pluginRoot 'extension/target/wasm32-unknown-unknown/release/photocraft_plugin_background_removal.wasm') -Destination (Join-Path $pluginStage 'plugin.pcext')
    Copy-Item -LiteralPath (Join-Path $pluginRoot 'runtime') -Destination $pluginStage -Recurse
    # DLLs imported by ONNX must be beside the worker for Windows app-local loading.
    $pluginVcRuntime = Get-Content -LiteralPath (Join-Path $pluginRoot 'runtime/vc-runtime.json') -Raw | ConvertFrom-Json
    foreach ($pluginAsset in $pluginVcRuntime.files | Where-Object { $_.path.StartsWith('bin/') }) {
        Move-Item -LiteralPath (Join-Path $pluginStage ('runtime/' + $pluginAsset.path)) -Destination $pluginStage
    }
    Copy-Item -Path (Join-Path $pluginRoot 'licenses/*') -Destination (Join-Path $pluginStage 'licenses') -Recurse -Force
    Copy-Item -LiteralPath (Join-Path $pluginRoot 'vendor/rust-background-removal/LICENSE') -Destination (Join-Path $pluginStage 'licenses/rust-background-removal-MIT.txt')

    # The .pcplugin is a ZIP with files directly at its root and portable slash-separated names.
    Add-Type -AssemblyName System.IO.Compression
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $pluginZip = [System.IO.Compression.ZipFile]::Open($pluginTemporary, [System.IO.Compression.ZipArchiveMode]::Create)
    try {
        foreach ($pluginFile in Get-ChildItem -LiteralPath $pluginStage -File -Recurse | Sort-Object FullName) {
            $pluginEntryName = $pluginFile.FullName.Substring($pluginStage.Length + 1).Replace('\', '/')
            $pluginEntry = $pluginZip.CreateEntry($pluginEntryName, [System.IO.Compression.CompressionLevel]::Optimal)
            $pluginEntry.LastWriteTime = [DateTimeOffset]::new(2026, 1, 1, 0, 0, 0, [TimeSpan]::Zero)
            $pluginInput = [System.IO.File]::OpenRead($pluginFile.FullName)
            try {
                $pluginOutput = $pluginEntry.Open()
                try { $pluginInput.CopyTo($pluginOutput) } finally { $pluginOutput.Dispose() }
            } finally { $pluginInput.Dispose() }
        }
    } finally { $pluginZip.Dispose() }
    Move-Item -LiteralPath $pluginTemporary -Destination $pluginArchive -Force
    Write-Output "Installable plugin: $pluginArchive"
    Write-Output 'In PhotoCraft: Plugins > Manage Plugins > Browse, then select this .pcplugin file. Older compatible builds use Choose plugin.'
} finally {
    Pop-Location
    # Only remove this invocation's staging directory after checking its resolved absolute boundary.
    $pluginStageResolved = [System.IO.Path]::GetFullPath($pluginStage)
    $pluginDistPrefix = [System.IO.Path]::GetFullPath($pluginDist).TrimEnd('\') + '\'
    if ($pluginStageResolved.StartsWith($pluginDistPrefix, [StringComparison]::OrdinalIgnoreCase) -and (Test-Path -LiteralPath $pluginStageResolved)) {
        Remove-Item -LiteralPath $pluginStageResolved -Recurse -Force
    }
    if (Test-Path -LiteralPath $pluginTemporary) { Remove-Item -LiteralPath $pluginTemporary -Force }
}
