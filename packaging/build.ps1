# Baut Playtube.exe mit PyInstaller und sorgt dafuer, dass auch der QtWebEngine-
# Hilfsprozess (der eigentlich den Ton ausgibt) den Namen "Playtube" traegt, damit er
# im Taskmanager und - so weit von der jeweiligen Windows-Version respektiert - im
# Lautstaerkemixer nicht als "QtWebEngineProcess" auftaucht.
#
# Aufruf (im Projekt-Root, mit aktivierter venv):
#   powershell -ExecutionPolicy Bypass -File packaging\build.ps1
#
# Optional: rcedit (https://github.com/electron/rcedit) im PATH oder unter
# packaging\rcedit.exe ablegen, dann werden Icon + Versionsinfo des umbenannten
# Hilfsprozesses ebenfalls auf "Playtube" gesetzt (sonst bleibt intern "QtWebEngineProcess"
# stehen, nur der Dateiname aendert sich - Windows zeigt dann meist trotzdem den
# Dateinamen "PlaytubeHelper" an).

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

Write-Host "==> Baue Playtube.exe mit PyInstaller ..." -ForegroundColor Cyan
pyinstaller packaging\playtube.spec --noconfirm --distpath dist --workpath build
if ($LASTEXITCODE -ne 0) { throw "PyInstaller-Build fehlgeschlagen." }

$distDir = Join-Path $root "dist\Playtube"
$internalDir = Join-Path $distDir "_internal"
$searchRoot = if (Test-Path $internalDir) { $internalDir } else { $distDir }

$engineProcess = Get-ChildItem -Path $searchRoot -Recurse -Filter "QtWebEngineProcess.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $engineProcess) {
    Write-Warning "QtWebEngineProcess.exe wurde im Build nicht gefunden - Audio-/Taskmanager-Branding des Hilfsprozesses wird uebersprungen."
} else {
    $helperPath = Join-Path $engineProcess.Directory.FullName "PlaytubeHelper.exe"
    Copy-Item $engineProcess.FullName $helperPath -Force
    Write-Host "==> Hilfsprozess kopiert nach $helperPath" -ForegroundColor Cyan

    $rcedit = Get-Command rcedit -ErrorAction SilentlyContinue
    $rceditLocal = Join-Path $PSScriptRoot "rcedit.exe"
    if ($rcedit) {
        $rceditExe = $rcedit.Source
    } elseif (Test-Path $rceditLocal) {
        $rceditExe = $rceditLocal
    } else {
        $rceditExe = $null
    }

    if ($rceditExe) {
        Write-Host "==> Patche Icon/Versionsinfo des Hilfsprozesses mit rcedit ..." -ForegroundColor Cyan
        & $rceditExe $helperPath --set-icon "$root\assets\icon.ico"
        & $rceditExe $helperPath --set-version-string "FileDescription" "Playtube"
        & $rceditExe $helperPath --set-version-string "ProductName" "Playtube"
        & $rceditExe $helperPath --set-version-string "CompanyName" "Playtube"
        & $rceditExe $helperPath --set-version-string "OriginalFilename" "PlaytubeHelper.exe"
    } else {
        Write-Host "Hinweis: rcedit nicht gefunden - Hilfsprozess heisst jetzt 'PlaytubeHelper.exe'," -ForegroundColor Yellow
        Write-Host "seine interne Versionsinfo/Icon bleibt aber 'QtWebEngineProcess'. Fuer volles" -ForegroundColor Yellow
        Write-Host "Branding: rcedit von https://github.com/electron/rcedit/releases laden und" -ForegroundColor Yellow
        Write-Host "als packaging\rcedit.exe ablegen, dann dieses Skript erneut ausfuehren." -ForegroundColor Yellow
    }
}

Write-Host ""
Write-Host "Fertig! Die App liegt unter dist\Playtube\Playtube.exe" -ForegroundColor Green
