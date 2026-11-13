# Baut das Playtube-Setup für ein Release:
#   dist\Playtube-Setup-<Version>.exe   (Installer, auch für große Updates aus der App)
#   dist\Playtube-<Version>-win64.exe    (nackte Programmdatei für kleine Updates ohne Setup)
# Die Version kommt aus Cargo.toml.
param([string]$Iscc = "")

$ErrorActionPreference = "Stop"
$root = $PSScriptRoot
$version = (Select-String -Path (Join-Path $root "Cargo.toml") -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
Write-Host "Playtube $version" -ForegroundColor Yellow

if (-not $Iscc) {
    $Iscc = @(
        "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe",
        "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
        "$env:ProgramFiles\Inno Setup 6\ISCC.exe"
    ) | Where-Object { Test-Path $_ } | Select-Object -First 1
}
if (-not $Iscc) { throw "Inno Setup 6 nicht gefunden (winget install JRSoftware.InnoSetup)" }

Push-Location $root
try {
    # Cargo schreibt Fortschritt nach stderr – nur der Exitcode zählt
    $ErrorActionPreference = "Continue"
    cargo build --release 2>&1 | ForEach-Object { "$_" } | Where-Object { $_ -match "error|warning|Finished" }
    $ErrorActionPreference = "Stop"
    if ($LASTEXITCODE -ne 0) { throw "cargo build fehlgeschlagen" }
} finally { Pop-Location }

$exe = Join-Path $root "target\release\playtube.exe"
$dist = Join-Path $root "dist"
New-Item -ItemType Directory -Force $dist | Out-Null

Copy-Item $exe (Join-Path $dist "Playtube-$version-win64.exe") -Force

& $Iscc /Q "/DAppVersion=$version" "/DSourceExe=$exe" (Join-Path $root "installer\Playtube.iss")
if ($LASTEXITCODE -ne 0) { throw "Inno Setup fehlgeschlagen" }

Get-ChildItem $dist -Filter "*$version*" | ForEach-Object { "{0}  {1:N1} MB" -f $_.Name, ($_.Length / 1MB) }
