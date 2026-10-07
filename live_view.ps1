# Tuquet Live View Inspection Launcher (Runner Workspace)
# Strict ASCII encoding - no diacritics to avoid Windows PowerShell parsing issues

$ErrorActionPreference = 'Stop'

Write-Host "==========================================" -ForegroundColor Cyan
Write-Host " [TUQUET] Live View Visual Inspection" -ForegroundColor Cyan
Write-Host "==========================================" -ForegroundColor Cyan

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
Push-Location $ScriptDir
try {
    cargo run --example live_view
} finally {
    Pop-Location
}
