param([switch]$Offline)
$ErrorActionPreference = 'Stop'
$pluginRoot = [IO.Path]::GetFullPath((Split-Path $PSScriptRoot))
$runtimeRoot = Join-Path $pluginRoot 'runtime'
$runtimePrefix = [IO.Path]::GetFullPath($runtimeRoot).TrimEnd('\') + '\'
$cacheRoot = Join-Path $pluginRoot 'target/download-cache'
$manifest = Get-Content -LiteralPath (Join-Path $runtimeRoot 'downloads.json') -Raw | ConvertFrom-Json
New-Item -ItemType Directory -Path $cacheRoot -Force | Out-Null
Add-Type -AssemblyName System.IO.Compression.FileSystem

foreach ($asset in $manifest.files) {
    $destination = [IO.Path]::GetFullPath((Join-Path $runtimeRoot $asset.path))
    if (-not $destination.StartsWith($runtimePrefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'An asset path escapes the runtime directory.'
    }
    if ((Test-Path -LiteralPath $destination -PathType Leaf) -and
        (Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash.ToLowerInvariant() -eq $asset.sha256) {
        Write-Output "Verified $($asset.path)"
        continue
    }
    New-Item -ItemType Directory -Path (Split-Path $destination) -Force | Out-Null
    $temporary = $destination + '.part'
    try {
        if ($Offline) { throw "Missing or altered cached asset $($asset.path). Run fetch-assets.ps1 online first." }
        if ($asset.archivePath) {
            $archive = Join-Path $cacheRoot 'onnxruntime-win-x64-1.22.0.zip'
            if (-not (Test-Path -LiteralPath $archive -PathType Leaf) -or
                (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $manifest.onnxRuntimeArchiveSha256) {
                Invoke-WebRequest -Uri $asset.url -OutFile $archive -UserAgent 'Photocraft-dev'
            }
            if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $manifest.onnxRuntimeArchiveSha256) {
                throw 'ONNX Runtime archive checksum mismatch.'
            }
            $zip = [IO.Compression.ZipFile]::OpenRead($archive)
            try {
                $entry = $zip.GetEntry($asset.archivePath)
                if ($null -eq $entry) { throw "Missing archive member $($asset.archivePath)" }
                $inputStream = $entry.Open()
                try {
                    $outputStream = [IO.File]::Create($temporary)
                    try { $inputStream.CopyTo($outputStream) } finally { $outputStream.Dispose() }
                } finally { $inputStream.Dispose() }
            } finally { $zip.Dispose() }
        } else {
            Invoke-WebRequest -Uri $asset.url -OutFile $temporary -UserAgent 'Photocraft-dev'
        }
        if ((Get-FileHash -LiteralPath $temporary -Algorithm SHA256).Hash.ToLowerInvariant() -ne $asset.sha256) {
            throw "Checksum mismatch: $($asset.path)"
        }
        Move-Item -LiteralPath $temporary -Destination $destination -Force
        Write-Output "Downloaded and verified $($asset.path)"
    } finally {
        if (Test-Path -LiteralPath $temporary -PathType Leaf) { Remove-Item -LiteralPath $temporary -Force }
    }
}

& (Join-Path $PSScriptRoot 'fetch-vc-runtime.ps1') -Offline:$Offline
