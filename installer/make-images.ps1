# Zeichnet die Bilder für das Setup (Inno Setup, Stil "modern"):
#   wizard-<n>.bmp  großes Bild links auf Willkommens-/Fertig-Seite (164x314 bei 100 %)
#   small-<n>.bmp   kleines Logo oben rechts auf den übrigen Seiten (55x55 bei 100 %)
# jeweils für 100 bis 200 % Anzeigeskalierung. Aufruf: powershell -File make-images.ps1
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing
$out = $PSScriptRoot

$purple = [System.Drawing.Color]::FromArgb(124, 58, 237)
$pink = [System.Drawing.Color]::FromArgb(236, 72, 153)
$dark = [System.Drawing.Color]::FromArgb(15, 15, 20)

function New-RoundedRect([float]$x, [float]$y, [float]$w, [float]$h, [float]$r) {
    $p = New-Object System.Drawing.Drawing2D.GraphicsPath
    $d = 2 * $r
    $p.AddArc($x, $y, $d, $d, 180, 90)
    $p.AddArc($x + $w - $d, $y, $d, $d, 270, 90)
    $p.AddArc($x + $w - $d, $y + $h - $d, $d, $d, 0, 90)
    $p.AddArc($x, $y + $h - $d, $d, $d, 90, 90)
    $p.CloseFigure()
    return $p
}

# Playtube-Logo: abgerundetes Quadrat im Verlauf, Play-Dreieck, Note und zwei Schallwellen.
function Draw-Logo($g, [float]$x, [float]$y, [float]$size) {
    $rect = New-Object System.Drawing.RectangleF($x, $y, $size, $size)
    $grad = New-Object System.Drawing.Drawing2D.LinearGradientBrush($rect, $purple, $pink, 45)
    $shape = New-RoundedRect $x $y $size $size ($size * 0.24)
    $g.FillPath($grad, $shape)
    # leichter Glanz oben
    $shine = New-Object System.Drawing.Drawing2D.LinearGradientBrush($rect, [System.Drawing.Color]::FromArgb(60, 255, 255, 255), [System.Drawing.Color]::FromArgb(0, 255, 255, 255), 90)
    $g.FillPath($shine, $shape)

    $white = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::White)
    $s = $size
    $tri = [System.Drawing.PointF[]]@(
        (New-Object System.Drawing.PointF(($x + $s * 0.27), ($y + $s * 0.27))),
        (New-Object System.Drawing.PointF(($x + $s * 0.27), ($y + $s * 0.73))),
        (New-Object System.Drawing.PointF(($x + $s * 0.62), ($y + $s * 0.50)))
    )
    $g.FillPolygon($white, $tri)

    # Schallwellen rechts neben dem Dreieck
    $penW = [Math]::Max(1.0, $s * 0.045)
    $pen = New-Object System.Drawing.Pen([System.Drawing.Color]::FromArgb(230, 255, 255, 255), $penW)
    $pen.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
    $pen.EndCap = [System.Drawing.Drawing2D.LineCap]::Round
    foreach ($k in 1, 2) {
        $r = $s * (0.04 + 0.065 * $k)
        $cx = $x + $s * 0.62
        $cy = $y + $s * 0.50
        $g.DrawArc($pen, ($cx - $r + $s * 0.04), ($cy - $r), (2 * $r), (2 * $r), -40, 80)
    }

    # kleine Musiknote unten links
    $noteR = $s * 0.065
    $nx = $x + $s * 0.17
    $ny = $y + $s * 0.80
    $g.FillEllipse($white, ($nx - $noteR), ($ny - $noteR * 0.8), ($noteR * 2), ($noteR * 1.6))
    $stem = New-Object System.Drawing.Pen([System.Drawing.Color]::White, [Math]::Max(1.0, $s * 0.03))
    $g.DrawLine($stem, ($nx + $noteR * 0.85), $ny, ($nx + $noteR * 0.85), ($ny - $s * 0.22))
    $g.DrawLine($stem, ($nx + $noteR * 0.85), ($ny - $s * 0.22), ($nx + $noteR * 0.85 + $s * 0.08), ($ny - $s * 0.18))
}

function New-Canvas([int]$w, [int]$h) {
    $bmp = New-Object System.Drawing.Bitmap($w, $h, [System.Drawing.Imaging.PixelFormat]::Format24bppRgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::AntiAliasGridFit
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    return @($bmp, $g)
}

function Save-Bmp($bmp, [string]$name) {
    $bmp.Save((Join-Path $out $name), [System.Drawing.Imaging.ImageFormat]::Bmp)
    $bmp.Dispose()
}

foreach ($scale in 100, 125, 150, 175, 200) {
    $f = $scale / 100.0

    # ---------- großes Bild links ----------
    $w = [int](164 * $f); $h = [int](314 * $f)
    $bmp, $g = New-Canvas $w $h
    $g.Clear($dark)
    # weicher Lichtschein oben und unten in den Markenfarben
    foreach ($glow in @(@($purple, -0.35, -0.10), @($pink, 0.55, 0.70))) {
        $path = New-Object System.Drawing.Drawing2D.GraphicsPath
        $path.AddEllipse([float]($w * $glow[1]), [float]($h * $glow[2]), [float]($w * 1.1), [float]($w * 1.1))
        $pb = New-Object System.Drawing.Drawing2D.PathGradientBrush($path)
        $pb.CenterColor = [System.Drawing.Color]::FromArgb(150, $glow[0])
        $pb.SurroundColors = @([System.Drawing.Color]::FromArgb(0, $glow[0]))
        $g.FillPath($pb, $path)
    }
    # feine Wellenlinien als Hintergrundmuster
    $wave = New-Object System.Drawing.Pen([System.Drawing.Color]::FromArgb(28, 255, 255, 255), [float](1.2 * $f))
    for ($i = 0; $i -lt 7; $i++) {
        $pts = for ($xx = 0; $xx -le $w; $xx += [int](4 * $f)) {
            New-Object System.Drawing.PointF($xx, [float]($h * 0.62 + $i * 9 * $f + [Math]::Sin($xx / ($w / 6.0) + $i * 0.7) * 10 * $f))
        }
        $g.DrawCurve($wave, [System.Drawing.PointF[]]$pts)
    }
    $logo = 92 * $f
    Draw-Logo $g (($w - $logo) / 2) (58 * $f) $logo

    $titleFont = New-Object System.Drawing.Font("Segoe UI Semibold", [float](21 * $f), [System.Drawing.FontStyle]::Regular, [System.Drawing.GraphicsUnit]::Pixel)
    $subFont = New-Object System.Drawing.Font("Segoe UI", [float](10.5 * $f), [System.Drawing.FontStyle]::Regular, [System.Drawing.GraphicsUnit]::Pixel)
    $center = New-Object System.Drawing.StringFormat
    $center.Alignment = [System.Drawing.StringAlignment]::Center
    $g.DrawString("Playtube", $titleFont, [System.Drawing.Brushes]::White, (New-Object System.Drawing.RectangleF(0, [float](166 * $f), $w, [float](32 * $f))), $center)
    $soft = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(200, 235, 225, 255))
    $g.DrawString("YouTube & YouTube Music", $subFont, $soft, (New-Object System.Drawing.RectangleF(0, [float](198 * $f), $w, [float](18 * $f))), $center)
    # Akzentlinie im Verlauf
    $lineRect = New-Object System.Drawing.RectangleF([float]($w * 0.32), [float](224 * $f), [float]($w * 0.36), [float](3 * $f))
    $g.FillRectangle((New-Object System.Drawing.Drawing2D.LinearGradientBrush($lineRect, $purple, $pink, 0)), $lineRect)
    $g.Dispose()
    Save-Bmp $bmp "wizard-$scale.bmp"

    # ---------- kleines Logo oben rechts ----------
    $s = [int](55 * $f)
    $bmp, $g = New-Canvas $s $s
    $g.Clear([System.Drawing.Color]::White)
    Draw-Logo $g (3 * $f) (3 * $f) ($s - 6 * $f)
    $g.Dispose()
    Save-Bmp $bmp "small-$scale.bmp"
}
Get-ChildItem $out -Filter *.bmp | ForEach-Object { "{0}  {1}" -f $_.Name, $_.Length }
