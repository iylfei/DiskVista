param([string]$Version)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
if (-not $Version) {
    $Version = (Get-Content -LiteralPath (Join-Path $projectRoot 'apps/desktop/tauri.conf.json') -Encoding utf8 -Raw | ConvertFrom-Json).version
}
if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw 'Unexpected package version' }
$artifactRoot = Join-Path $projectRoot 'artifacts'
$portable = Join-Path $artifactRoot "DiskVista-$Version-win-x64"
$marker = Join-Path $portable '.generated-package'
if (Test-Path -LiteralPath $portable) {
    $resolved = [IO.Path]::GetFullPath($portable)
    $expected = [IO.Path]::GetFullPath($artifactRoot) + [IO.Path]::DirectorySeparatorChar
    if (-not $resolved.StartsWith($expected, [StringComparison]::OrdinalIgnoreCase) -or
        -not (Test-Path -LiteralPath $marker) -or
        (Get-Content -LiteralPath $marker -Encoding utf8 -Raw).Trim() -ne 'DiskVista generated portable package') {
        throw 'Refusing to replace an unrecognized portable directory'
    }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
New-Item -ItemType Directory -Path $portable -Force | Out-Null
[IO.File]::WriteAllText($marker, "DiskVista generated portable package`n", [Text.UTF8Encoding]::new($false))
foreach ($name in @('diskvista.exe', 'diskvista-worker.exe')) {
    Copy-Item -LiteralPath (Join-Path $artifactRoot $name) -Destination $portable
}
foreach ($name in @('README.md', 'third-party')) {
    Copy-Item -LiteralPath (Join-Path $projectRoot $name) -Destination $portable -Recurse
}
$zip = Join-Path $artifactRoot "DiskVista-$Version-win-x64.zip"
Compress-Archive -LiteralPath $portable -DestinationPath $zip -CompressionLevel Optimal -Force -WarningAction SilentlyContinue
$hashes = Get-ChildItem -LiteralPath $artifactRoot -File |
    Where-Object { $_.Extension -in @('.exe', '.zip') } |
    Sort-Object Name |
    Get-FileHash -Algorithm SHA256
$lines = @($hashes | ForEach-Object { $_.Hash.ToLowerInvariant() + '  ' + [IO.Path]::GetFileName($_.Path) })
[IO.File]::WriteAllLines((Join-Path $artifactRoot 'SHA256SUMS.txt'), $lines, [Text.UTF8Encoding]::new($false))
$hashes | Format-Table -AutoSize
