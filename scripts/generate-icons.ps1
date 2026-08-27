$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot

Push-Location -LiteralPath $projectRoot
try {
    pnpm exec tauri icon assets/branding/diskvista-icon.svg --output apps/desktop/icons
    if ($LASTEXITCODE -ne 0) { throw "Icon generation failed: $LASTEXITCODE" }
    Copy-Item -LiteralPath apps/desktop/icons/icon.png -Destination assets/branding/diskvista-icon.png -Force
    Copy-Item -LiteralPath apps/desktop/icons/128x128.png -Destination ui/public/app-icon.png -Force
    Copy-Item -LiteralPath apps/desktop/icons/icon.ico -Destination ui/public/favicon.ico -Force
} finally {
    Pop-Location
}
