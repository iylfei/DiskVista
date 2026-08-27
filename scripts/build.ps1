param([switch]$SkipChecks)
$ErrorActionPreference='Stop'
$projectRoot=Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $projectRoot
$env:Path=(Join-Path $env:USERPROFILE '.cargo/bin')+';'+$env:Path
function Invoke-Checked { param([string]$Command,[string[]]$Arguments) & $Command @Arguments; if($LASTEXITCODE -ne 0){throw "$Command failed: $LASTEXITCODE"} }
Invoke-Checked cargo @('build','--release','-p','diskvista-worker','--locked')
$binaries=Join-Path $projectRoot 'apps/desktop/binaries'
New-Item -ItemType Directory -Path $binaries -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $projectRoot 'target/release/diskvista-worker.exe') -Destination (Join-Path $binaries 'diskvista-worker-x86_64-pc-windows-msvc.exe') -Force
if(-not $SkipChecks){
 Invoke-Checked cargo @('fmt','--all','--','--check')
 Invoke-Checked cargo @('test','--workspace','--locked')
 Invoke-Checked cargo @('clippy','--workspace','--all-targets','--','-D','warnings')
 Invoke-Checked pnpm @('test')
 Invoke-Checked node @('--test','scripts/vendor-rules.test.mjs')
}
Invoke-Checked node @('scripts/collect-licenses.mjs')
Push-Location (Join-Path $projectRoot 'apps/desktop')
try {Invoke-Checked pnpm @('exec','tauri','build','--bundles','nsis')} finally {Pop-Location}
$out=Join-Path $projectRoot 'artifacts'
New-Item -ItemType Directory -Path $out -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $projectRoot 'target/release/diskvista.exe') -Destination $out -Force
Copy-Item -LiteralPath (Join-Path $projectRoot 'target/release/diskvista-worker.exe') -Destination $out -Force
$version=(Get-Content -LiteralPath (Join-Path $projectRoot 'apps/desktop/tauri.conf.json') -Encoding utf8 -Raw | ConvertFrom-Json).version
Copy-Item -LiteralPath (Join-Path $projectRoot "target/release/bundle/nsis/DiskVista_${version}_x64-setup.exe") -Destination $out -Force
& (Join-Path $PSScriptRoot 'package-portable.ps1')
