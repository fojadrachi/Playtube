# Veroeffentlicht ein Release auf dem eigenen Gitea-Server (Installer, ZIP, Patch als Anhaenge).
# Wird von der Pipeline (.gitea/workflows/release.yml) benutzt und laesst sich auch von Hand
# aufrufen, z.B. nach einem lokalen Build (packaging\build.ps1):
#
#   $env:GITEA_TOKEN = "<Zugriffstoken>"
#   powershell -ExecutionPolicy Bypass -File packaging\publish_release.ps1 `
#       -Tag v4.6.0 -Files dist\installer\Playtube-Setup-v4.6.0.exe,Playtube-v4.6.0-win64.zip
#
# Der Token wird in Gitea unter Einstellungen > Anwendungen erzeugt (Recht "repository: Lesen und
# Schreiben"). Er steht NIE im Skript, sondern nur in der Umgebungsvariable GITEA_TOKEN.
# Gab es zum selben Tag schon ein Release, wird es vorher entfernt (der Git-Tag bleibt).

param(
    [Parameter(Mandatory = $true)] [string]$Tag,
    [Parameter(Mandatory = $true)] [string[]]$Files,
    [string]$Title = "",
    [string]$NotesFile = "",
    [switch]$Prerelease,
    [string]$Server = "https://git.fojadrachi.de",
    [string]$Repo = "Fojadrachi/Playtube"
)

$ErrorActionPreference = "Stop"

$token = $env:GITEA_TOKEN
if (-not $token) {
    throw "GITEA_TOKEN ist nicht gesetzt (Zugriffstoken aus Gitea > Einstellungen > Anwendungen)."
}
if (-not $Title) { $Title = $Tag }
foreach ($file in $Files) {
    if (-not (Test-Path $file)) { throw "Datei nicht gefunden: $file" }
}

$api = "$Server/api/v1/repos/$Repo"
$headers = @{ Authorization = "token $token" }

# Vorhandenes Release zum selben Tag entfernen (404 = gab keins).
try {
    $old = Invoke-RestMethod -Headers $headers -Uri "$api/releases/tags/$Tag"
    Invoke-RestMethod -Method Delete -Headers $headers -Uri "$api/releases/$($old.id)" | Out-Null
    Write-Host "Vorhandenes Release zu $Tag ersetzt."
} catch {
    if ($_.Exception.Response -and [int]$_.Exception.Response.StatusCode -ne 404) { throw }
}

$notes = ""
if ($NotesFile -and (Test-Path $NotesFile)) { $notes = Get-Content -Raw -Encoding UTF8 $NotesFile }

$payload = @{
    tag_name   = $Tag
    name       = $Title
    body       = $notes
    draft      = $false
    prerelease = [bool]$Prerelease
} | ConvertTo-Json
$release = Invoke-RestMethod -Method Post -Headers $headers -Uri "$api/releases" `
    -ContentType "application/json; charset=utf-8" -Body ([Text.Encoding]::UTF8.GetBytes($payload))

foreach ($file in $Files) {
    $name = Split-Path $file -Leaf
    Write-Host "Lade hoch: $name"
    & curl.exe -sS --fail -X POST -H "Authorization: token $token" -F "attachment=@$file" `
        "$api/releases/$($release.id)/assets?name=$([uri]::EscapeDataString($name))" | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "Upload fehlgeschlagen: $name" }
}

Write-Host "Fertig: $Server/$Repo/releases/tag/$Tag" -ForegroundColor Green
