# Veroeffentlicht eine neue Playtube-Version: setzt die Versionsnummer, committet,
# taggt und pusht. Der eigentliche Build fuer Windows UND Linux sowie die
# Veroeffentlichung als GitHub Release passiert danach automatisch per GitHub Actions
# (.github/workflows/release.yml), ausgeloest durch den gepushten Tag.
#
# Aufruf (im Projekt-Root):
#   powershell -ExecutionPolicy Bypass -File packaging\release.ps1 -Version 1.1.0
#
# Voraussetzung: git remote "origin" zeigt auf https://github.com/fojadrachi/Playtube
# und du bist dort push-berechtigt.

param(
    [Parameter(Mandatory = $true)]
    [string]$Version
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

if ($Version -notmatch '^\d+\.\d+\.\d+$') {
    throw "Version muss im Format X.Y.Z angegeben werden, z.B. 1.1.0"
}
$tag = "v$Version"

Write-Host "==> Setze Version auf $Version in playtube/__init__.py" -ForegroundColor Cyan
$initFile = Join-Path $root "playtube\__init__.py"
(Get-Content $initFile) -replace '__version__ = ".*"', "__version__ = `"$Version`"" | Set-Content $initFile -Encoding utf8

Write-Host "==> Commit + Tag $tag ..." -ForegroundColor Cyan
git add -A
git commit -m "Release $tag" --allow-empty
git tag -a $tag -m "Playtube $tag"

Write-Host "==> Push zu origin ..." -ForegroundColor Cyan
git push origin HEAD
git push origin $tag

Write-Host ""
Write-Host "Fertig. GitHub Actions baut jetzt Windows- und Linux-Pakete und" -ForegroundColor Green
Write-Host "veroeffentlicht sie als Release $tag - Fortschritt unter:" -ForegroundColor Green
Write-Host "https://github.com/fojadrachi/Playtube/actions" -ForegroundColor Green
