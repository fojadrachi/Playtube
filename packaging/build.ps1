# Baut PlaytubeEdge.exe (Playtube mit Microsoft-Edge-Engine / WebView2) mit PyInstaller und
# danach den Setup-Installer (Inno Setup 6).
#
# Aufruf (im Projekt-Root, mit aktivierter venv):
#   powershell -ExecutionPolicy Bypass -File packaging\build.ps1

param(
    # Ueberspringt den Bau des Setup-Installers (PlaytubeEdge-Setup-vX.Y.Z.exe, benoetigt Inno Setup 6).
    [switch]$SkipInstaller
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

Write-Host "==> Baue PlaytubeEdge.exe mit PyInstaller ..." -ForegroundColor Cyan
pyinstaller packaging\playtube.spec --noconfirm --distpath dist --workpath build
if ($LASTEXITCODE -ne 0) { throw "PyInstaller-Build fehlgeschlagen." }
Write-Host "Fertig! Die App liegt unter dist\PlaytubeEdge\PlaytubeEdge.exe" -ForegroundColor Green

# --- Setup-Installer (Inno Setup) ------------------------------------------------------
# Ein und derselbe Installer richtet Playtube Edge erstmalig ein UND aktualisiert spaeter eine
# vorhandene Installation an Ort und Stelle (feste AppId) - es gibt nie mehrere Versionen.
if (-not $SkipInstaller) {
    $versionLine = Select-String -Path (Join-Path $root "playtube\__init__.py") -Pattern '__version__\s*=\s*"([^"]+)"' | Select-Object -First 1
    if (-not $versionLine) { throw "Konnte __version__ in playtube\__init__.py nicht lesen." }
    $version = $versionLine.Matches[0].Groups[1].Value

    $isccCandidates = @(
        (Get-Command iscc -ErrorAction SilentlyContinue | ForEach-Object { $_.Source }),
        (Join-Path $env:LOCALAPPDATA "Programs\Inno Setup 6\ISCC.exe"),
        (Join-Path ${env:ProgramFiles(x86)} "Inno Setup 6\ISCC.exe"),
        (Join-Path $env:ProgramFiles "Inno Setup 6\ISCC.exe")
    )
    $iscc = $isccCandidates | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1

    if (-not $iscc) {
        Write-Host ""
        Write-Host "Hinweis: Inno Setup 6 nicht gefunden - Setup-Installer wird uebersprungen." -ForegroundColor Yellow
        Write-Host "Installieren:  winget install --id JRSoftware.InnoSetup -e" -ForegroundColor Yellow
    } else {
        Write-Host ""
        Write-Host "==> Baue Setup-Installer (Version $version) mit Inno Setup ..." -ForegroundColor Cyan
        & $iscc "/DAppVersion=$version" (Join-Path $PSScriptRoot "playtube.iss")
        if ($LASTEXITCODE -ne 0) { throw "Inno-Setup-Build fehlgeschlagen." }
        Write-Host "Fertig! Installer: dist\installer\PlaytubeEdge-Setup-v$version.exe" -ForegroundColor Green
    }
}
