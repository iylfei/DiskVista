$ErrorActionPreference='Stop'
$projectRoot=Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $projectRoot
$env:Path=(Join-Path $env:USERPROFILE '.cargo/bin')+';'+$env:Path
cargo build -p diskvista-worker
if($LASTEXITCODE -ne 0){throw 'Worker build failed'}
New-Item -ItemType Directory -Path apps/desktop/binaries -Force | Out-Null
Copy-Item -LiteralPath target/debug/diskvista-worker.exe -Destination apps/desktop/binaries/diskvista-worker-x86_64-pc-windows-msvc.exe -Force
Push-Location apps/desktop
try {pnpm exec tauri dev} finally {Pop-Location}
