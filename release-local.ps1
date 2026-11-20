# Veröffentlicht ein Playtube-Release mit dem Windows-Build von diesem PC:
#   1. Tests (cargo test --locked)
#   2. Setup und win64.exe bauen (build-release.ps1)
#   3. main pushen, Tag v<Version> anlegen und pushen
#   4. GitHub-Release anlegen (Titel und Notizen wie im Workflow) und die Windows-Dateien hochladen
# Linux baut weiter GitHub (.github/workflows/release.yml) und hängt .deb und AppImage an dasselbe Release.
# Vorher: Version in Cargo.toml erhöhen, release_notes.md schreiben, alles committen.
# Der GitHub-Zugang kommt aus der Git-Anmeldung (git credential fill).

# -SkipBuild: nur hochladen (schon gebaut, z. B. nach einem Fehler beim Hochladen).
param([switch]$SkipBuild)

$ErrorActionPreference = "Stop"
$root = $PSScriptRoot
$repo = "fojadrachi/Playtube"
Set-Location $root

$version = (Select-String -Path "Cargo.toml" -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
$tag = "v$version"
Write-Host "Playtube $version – lokales Windows-Release" -ForegroundColor Yellow

function Invoke-Git { & git.exe -c safe.directory=* @args; if ($LASTEXITCODE -ne 0) { throw "git $args fehlgeschlagen" } }

# Nichts Uncommittetes mitveröffentlichen.
$dirty = & git -c safe.directory=* status --porcelain --untracked-files=no
if ($dirty) { throw "Es gibt nicht committete Änderungen:`n$dirty" }

# 1. Tests
if (-not $SkipBuild) {
$ErrorActionPreference = "Continue"
cargo test --locked 2>&1 | ForEach-Object { "$_" } | Where-Object { $_ -match "^test result|FAILED|panicked|^error" }
$ErrorActionPreference = "Stop"
if ($LASTEXITCODE -ne 0) { throw "Tests fehlgeschlagen" }

# 2. Bauen
& (Join-Path $root "build-release.ps1")
}
$files = @("dist\Playtube-Setup-$version.exe", "dist\Playtube-$version-win64.exe") | ForEach-Object { Join-Path $root $_ }
foreach ($f in $files) { if (-not (Test-Path $f)) { throw "Fehlt: $f" } }

# 3. Pushen und taggen (der Tag startet den Linux-Build auf GitHub)
Invoke-Git push -q origin HEAD:main
$existing = & git -c safe.directory=* ls-remote --tags origin "refs/tags/$tag"
if (-not $existing) {
    $local = & git -c safe.directory=* tag -l $tag
    if (-not $local) { Invoke-Git tag $tag }
    Invoke-Git push -q origin $tag
}

# 4. Release anlegen (oder das vom Workflow schon angelegte nehmen) und Dateien hochladen
# GitHub-Zugang aus der Git-Anmeldung (direkt über stdin, PowerShell-Pipes stören git hier).
function Get-GitHubToken {
    $psi = New-Object System.Diagnostics.ProcessStartInfo "git.exe", "credential fill"
    $psi.RedirectStandardInput = $true; $psi.RedirectStandardOutput = $true; $psi.UseShellExecute = $false
    $p = [System.Diagnostics.Process]::Start($psi)
    $p.StandardInput.NewLine = "`n"
    $p.StandardInput.WriteLine("protocol=https"); $p.StandardInput.WriteLine("host=github.com"); $p.StandardInput.WriteLine("")
    $p.StandardInput.Close()
    $out = $p.StandardOutput.ReadToEnd(); $p.WaitForExit()
    ($out -split "`n" | Where-Object { $_ -like "password=*" } | Select-Object -First 1) -replace "^password=", ""
}
$token = (Get-GitHubToken).Trim()
if (-not $token) { throw "Kein GitHub-Zugang in der Git-Anmeldung gefunden" }
$headers = @{ Authorization = "Bearer $token"; Accept = "application/vnd.github+json"; "X-GitHub-Api-Version" = "2022-11-28" }
$api = "https://api.github.com/repos/$repo"

$release = $null
try { $release = Invoke-RestMethod -Headers $headers -Uri "$api/releases/tags/$tag" } catch { }
if (-not $release) {
    $notes = Get-Content (Join-Path $root "release_notes.md") -Raw -Encoding utf8
    $body = @{ tag_name = $tag; name = "Playtube $tag"; body = $notes; make_latest = "true" } | ConvertTo-Json
    $release = Invoke-RestMethod -Method Post -Headers $headers -Uri "$api/releases" -Body ([Text.Encoding]::UTF8.GetBytes($body)) -ContentType "application/json; charset=utf-8"
}

foreach ($f in $files) {
    $name = Split-Path $f -Leaf
    $old = $release.assets | Where-Object { $_.name -eq $name }
    if ($old) { Invoke-RestMethod -Method Delete -Headers $headers -Uri "$api/releases/assets/$($old.id)" | Out-Null }
    $url = "https://uploads.github.com/repos/$repo/releases/$($release.id)/assets?name=$([uri]::EscapeDataString($name))"
    Invoke-RestMethod -Method Post -Headers $headers -Uri $url -InFile $f -ContentType "application/octet-stream" | Out-Null
    Write-Host "hochgeladen: $name"
}

Write-Host "Fertig: $($release.html_url)" -ForegroundColor Green
Write-Host "Linux (.deb, AppImage) hängt GitHub in ein paar Minuten an: https://github.com/$repo/actions"
