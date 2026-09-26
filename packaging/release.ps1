# Veroeffentlicht eine neue Playtube-Edge-Version: setzt die Versionsnummer, committet,
# taggt ("vX.Y.Z-edge") und pusht. Build und Veroeffentlichung als Vorabversion auf dem
# eigenen Gitea-Server passieren danach automatisch per Gitea Actions
# (.gitea/workflows/release.yml), ausgeloest durch den gepushten Tag.
#
# Aufruf (im Projekt-Root, auf dem Branch "edge"):
#   powershell -ExecutionPolicy Bypass -File packaging\release.ps1 -Version 1.0.1
#
# Voraussetzung: git remote "origin" zeigt auf https://git.fojadrachi.de/Fojadrachi/Playtube
# und du bist dort push-berechtigt. Ohne Actions-Runner kannst du lokal bauen
# (packaging\build.ps1) und mit packaging\publish_release.ps1 -Prerelease selbst veroeffentlichen.

param(
    [Parameter(Mandatory = $true)]
    [string]$Version
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

if ($Version -notmatch '^\d+\.\d+\.\d+$') {
    throw "Version muss im Format X.Y.Z angegeben werden, z.B. 1.0.1"
}
$tag = "v$Version-edge"

Write-Host "==> Setze Version auf $Version in playtube/__init__.py" -ForegroundColor Cyan
$initFile = Join-Path $root "playtube\__init__.py"
(Get-Content $initFile) -replace '__version__ = ".*"', "__version__ = `"$Version`"" | Set-Content $initFile -Encoding utf8

Write-Host "==> Commit + Tag $tag ..." -ForegroundColor Cyan
git add -A
git commit -m "Release $tag" --allow-empty
git tag -a $tag -m "Playtube Edge $tag"

Write-Host "==> Push zu origin ..." -ForegroundColor Cyan
git push origin HEAD
git push origin $tag

Write-Host ""
Write-Host "Fertig. Gitea Actions baut jetzt Playtube Edge und" -ForegroundColor Green
Write-Host "veroeffentlicht es als Release $tag - Fortschritt unter:" -ForegroundColor Green
Write-Host "https://git.fojadrachi.de/Fojadrachi/Playtube/actions" -ForegroundColor Green
