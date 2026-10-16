param([switch]$Offline)
$ErrorActionPreference = 'Stop'
$pluginRoot = [IO.Path]::GetFullPath((Split-Path $PSScriptRoot))
$runtimeRoot = Join-Path $pluginRoot 'runtime'
$runtimePrefix = $runtimeRoot.TrimEnd('\') + '\'
$cacheRoot = Join-Path $pluginRoot 'target/download-cache'
$manifest = Get-Content -LiteralPath (Join-Path $runtimeRoot 'vc-runtime.json') -Raw | ConvertFrom-Json
$missing = @($manifest.files | Where-Object {
    $assetPath = [IO.Path]::GetFullPath((Join-Path $runtimeRoot $_.path))
    if (-not $assetPath.StartsWith($runtimePrefix, [StringComparison]::OrdinalIgnoreCase)) { throw 'Runtime path escapes package.' }
    -not (Test-Path -LiteralPath $assetPath -PathType Leaf) -or (Get-FileHash -LiteralPath $assetPath -Algorithm SHA256).Hash.ToLowerInvariant() -ne $_.sha256
})
if ($missing.Count -eq 0) { Write-Output 'Verified app-local Microsoft VC runtime.'; return }
New-Item -ItemType Directory -Path $cacheRoot -Force | Out-Null
$archive = Join-Path $cacheRoot 'vc-redist-x64-14.44.35211.exe'
if (-not (Test-Path -LiteralPath $archive -PathType Leaf) -or
    (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $manifest.archiveSha256) {
    if ($Offline) { throw 'Pinned VC runtime download is not cached. Run fetch-assets.ps1 online first.' }
    Invoke-WebRequest -Uri $manifest.url -OutFile $archive -UserAgent 'Photocraft-plugin-dev'
}
if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $manifest.archiveSha256) { throw 'VC runtime archive checksum mismatch.' }
# CAB support is provided by Windows' libarchive-based tar. GNU tar from a Rust
# developer PATH does not support cabinets, so never resolve this tool via PATH.
$cabTar = Join-Path ([Environment]::GetFolderPath([Environment+SpecialFolder]::Windows)) 'System32/tar.exe'
if (-not (Test-Path -LiteralPath $cabTar -PathType Leaf)) { throw 'Windows system tar.exe is required to extract the pinned VC runtime cabinets.' }
$extractRoot = Join-Path $cacheRoot ('vc-extract-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $extractRoot -Force | Out-Null
try {
    # Extract data only. Never execute the redistributable or modify the system runtime.
    $bytes = [IO.File]::ReadAllBytes($archive)
    foreach ($container in @('ux', 'payload')) {
        $offset = [int]$manifest.($container + 'Offset')
        $length = [int]$manifest.($container + 'Size')
        if ($offset -lt 0 -or $length -lt 36 -or $offset -gt $bytes.Length - $length) { throw 'Invalid pinned cabinet range.' }
        $cabinet = [byte[]]::new($length)
        [Array]::Copy($bytes, $offset, $cabinet, 0, $length)
        $cabinetPath = Join-Path $extractRoot ($container + '.cab')
        [IO.File]::WriteAllBytes($cabinetPath, $cabinet)
        $member = if ($container -eq 'ux') { 'u4' } else { $manifest.minimumMember }
        & $cabTar -xf $cabinetPath -C $extractRoot $member
        if ($LASTEXITCODE -ne 0) { throw 'VC runtime cabinet extraction failed.' }
    }
    $minimumCabinet = Join-Path $extractRoot $manifest.minimumMember
    $members = @($manifest.files | Where-Object { $_.container -ne 'ux' } | ForEach-Object { $_.member })
    & $cabTar -xf $minimumCabinet -C $extractRoot @members
    if ($LASTEXITCODE -ne 0) { throw 'VC runtime DLL extraction failed.' }
    foreach ($asset in $manifest.files) {
        $source = Join-Path $extractRoot $asset.member
        if ((Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash.ToLowerInvariant() -ne $asset.sha256) { throw "VC runtime checksum mismatch: $($asset.path)" }
        $destination = [IO.Path]::GetFullPath((Join-Path $runtimeRoot $asset.path))
        if (-not $destination.StartsWith($runtimePrefix, [StringComparison]::OrdinalIgnoreCase)) { throw 'Runtime path escapes package.' }
        New-Item -ItemType Directory -Path (Split-Path $destination) -Force | Out-Null
        Copy-Item -LiteralPath $source -Destination $destination -Force
        Write-Output "Restored $($asset.path)"
    }
} finally {
    $cachePrefix = [IO.Path]::GetFullPath($cacheRoot).TrimEnd('\') + '\'
    $extractResolved = [IO.Path]::GetFullPath($extractRoot)
    if ($extractResolved.StartsWith($cachePrefix, [StringComparison]::OrdinalIgnoreCase) -and (Test-Path -LiteralPath $extractResolved)) {
        Remove-Item -LiteralPath $extractResolved -Recurse -Force
    }
}
