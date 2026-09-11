# Baut eine neue Playtube-Version, taggt und veroeffentlicht sie als GitHub Release
# unter https://github.com/fojadrachi/Playtube - Playtube.exe erkennt neue Releases
# danach automatisch (siehe playtube/updater.py) und bietet dem Nutzer die Installation
# per Update-Dialog an.
#
# Aufruf (im Projekt-Root, mit aktivierter venv):
#   powershell -ExecutionPolicy Bypass -File packaging\release.ps1 -Version 1.1.0
#
# Voraussetzungen:
#   - git remote "origin" zeigt auf https://github.com/fojadrachi/Playtube
#   - Entweder die GitHub-CLI "gh" ist installiert und eingeloggt (gh auth login),
#     oder die Umgebungsvariable GITHUB_TOKEN enthaelt ein Personal Access Token mit
#     "repo"-Rechten (dann wird die GitHub REST API direkt per curl angesprochen).

param(
    [Parameter(Mandatory = $true)]
    [string]$Version,
    [string]$Notes = ""
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

Write-Host "==> Baue Playtube.exe ..." -ForegroundColor Cyan
& (Join-Path $PSScriptRoot "build.ps1")

$distDir = Join-Path $root "dist\Playtube"
if (-not (Test-Path $distDir)) { throw "Build fehlgeschlagen: $distDir nicht gefunden." }

$zipName = "Playtube-$tag-win64.zip"
$zipPath = Join-Path $root "dist\$zipName"
if (Test-Path $zipPath) { Remove-Item $zipPath -Force }

Write-Host "==> Packe $zipName ..." -ForegroundColor Cyan
Compress-Archive -Path $distDir -DestinationPath $zipPath

Write-Host "==> Commit + Tag $tag ..." -ForegroundColor Cyan
git add -A
git commit -m "Release $tag" --allow-empty
git tag -a $tag -m "Playtube $tag"

Write-Host "==> Push zu origin ..." -ForegroundColor Cyan
git push origin HEAD
git push origin $tag

$gh = Get-Command gh -ErrorAction SilentlyContinue
if ($gh) {
    Write-Host "==> Erstelle GitHub Release mit gh CLI ..." -ForegroundColor Cyan
    if ([string]::IsNullOrWhiteSpace($Notes)) { $Notes = "Playtube $tag" }
    gh release create $tag $zipPath --title "Playtube $Version" --notes $Notes
} elseif ($env:GITHUB_TOKEN) {
    Write-Host "==> Erstelle GitHub Release ueber die REST API ..." -ForegroundColor Cyan
    $repo = "fojadrachi/Playtube"
    $body = @{ tag_name = $tag; name = "Playtube $Version"; body = $(if ($Notes) { $Notes } else { "Playtube $tag" }) } | ConvertTo-Json
    $headers = @{ Authorization = "Bearer $($env:GITHUB_TOKEN)"; Accept = "application/vnd.github+json" }
    $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$repo/releases" -Method Post -Headers $headers -Body $body -ContentType "application/json"
    $uploadUrl = $release.upload_url -replace '\{\?name,label\}', "?name=$zipName"
    Invoke-RestMethod -Uri $uploadUrl -Method Post -Headers ($headers + @{ "Content-Type" = "application/zip" }) -InFile $zipPath | Out-Null
    Write-Host "Release veroeffentlicht: $($release.html_url)" -ForegroundColor Green
} else {
    Write-Warning "Weder 'gh' CLI noch GITHUB_TOKEN gefunden - Tag wurde gepusht, aber kein GitHub Release erstellt."
    Write-Warning "Entweder 'winget install GitHub.cli' + 'gh auth login' ausfuehren und dieses Skript erneut starten,"
    Write-Warning "oder manuell unter https://github.com/fojadrachi/Playtube/releases/new ein Release fuer Tag '$tag' anlegen und '$zipPath' als Asset anhaengen."
}

Write-Host ""
Write-Host "Fertig." -ForegroundColor Green
