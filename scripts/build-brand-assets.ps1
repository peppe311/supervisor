#requires -Version 5.1
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$assetRoot = Join-Path $projectRoot 'assets'
$themes = Get-Content -LiteralPath (Join-Path $assetRoot 'themes.css') -Raw
function Get-BrandColor([string]$Token, [string]$Theme) {
    $palette = [regex]::Match($themes, '(?s):root\[data-theme="' + [regex]::Escape($Theme) + '"\]\s*\{(.*?)\}').Groups[1].Value
    $literal = [regex]::Match($palette, [regex]::Escape($Token) + ':\s*(#[0-9a-fA-F]{6})\s*;').Groups[1].Value
    if (-not $literal) { throw "The shared theme must provide a literal $Token for the native icon." }
    return [System.Drawing.ColorTranslator]::FromHtml($literal)
}

# Filled, closed polygons only. Reject unsupported commands instead of silently
# exporting incomplete artwork after a future SVG edit.
function Read-MarkPaths([string]$FileName) {
    [xml]$svg = Get-Content -LiteralPath (Join-Path $assetRoot $FileName) -Raw
    if ($svg.svg.fill -ne 'currentColor' -or @($svg.svg.path).Count -ne 2) {
        throw "Expected two filled Regia paths in $FileName."
    }
    foreach ($element in $svg.svg.path) {
        if ($element.d -match '[^MLHVZ0-9.,\s-]' -or $element.d -notmatch '^M.*Z$') {
            throw "Unsupported Regia path in $FileName."
        }
        $points = [System.Collections.Generic.List[System.Drawing.PointF]]::new()
        $x = [single]0; $y = [single]0
        foreach ($segment in [regex]::Matches($element.d, '([MLHVZ])([^MLHVZ]*)')) {
            if ($segment.Groups[1].Value -eq 'Z') { continue }
            $values = @([regex]::Matches($segment.Groups[2].Value, '-?\d+(?:\.\d+)?') | ForEach-Object {
                [single]::Parse($_.Value, [Globalization.CultureInfo]::InvariantCulture)
            })
            switch ($segment.Groups[1].Value) {
                { $_ -in 'M', 'L' } {
                    if ($values.Count -ne 2) { throw 'Unsupported Regia point.' }
                    $x = $values[0]; $y = $values[1]
                }
                'H' {
                    if ($values.Count -ne 1) { throw 'Unsupported Regia horizontal segment.' }
                    $x = $values[0]
                }
                'V' {
                    if ($values.Count -ne 1) { throw 'Unsupported Regia vertical segment.' }
                    $y = $values[0]
                }
            }
            $points.Add([System.Drawing.PointF]::new($x, $y))
        }
        $path = [System.Drawing.Drawing2D.GraphicsPath]::new()
        $path.AddPolygon($points.ToArray())
        $path
    }
}

$paths = @(Read-MarkPaths 'supervisor-mark.svg')
$opticalPaths = @(Read-MarkPaths 'supervisor-mark-native-16.svg')
try {
    foreach ($variant in @(
        @{ Name = 'supervisor-app-icon'; Ink = (Get-BrandColor '--ca-brand-file-ink' 'central') },
        @{ Name = 'supervisor-window-light'; Ink = (Get-BrandColor '--ca-brand-ink' 'central') },
        @{ Name = 'supervisor-window-dark'; Ink = (Get-BrandColor '--ca-brand-ink' 'central_dark') }
    )) {
        $ink = $variant.Ink
        $images = @()
        foreach ($size in @(16, 24, 32, 48, 64, 128, 256)) {
            # Each frame is independently rendered from vector geometry.
            $sample = 8
            $bitmap = [System.Drawing.Bitmap]::new($size * $sample, $size * $sample)
            $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
            $markBrush = [System.Drawing.SolidBrush]::new($ink)
            $stream = [System.IO.MemoryStream]::new()
            $small = $null; $smallGraphics = $null
            try {
                $graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
                $graphics.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::Half
                $graphics.ScaleTransform($sample, $sample)
                if ($size -eq 16) {
                    $framePaths = $opticalPaths
                } else {
                    $framePaths = $paths
                    $graphics.ScaleTransform($size / 64.0, $size / 64.0)
                }
                foreach ($path in $framePaths) { $graphics.FillPath($markBrush, $path) }
                $small = [System.Drawing.Bitmap]::new($size, $size)
                $smallGraphics = [System.Drawing.Graphics]::FromImage($small)
                $smallGraphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
                $smallGraphics.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::Half
                $smallGraphics.DrawImage($bitmap, 0, 0, $size, $size)
                $small.Save($stream, [System.Drawing.Imaging.ImageFormat]::Png)
                $bytes = $stream.ToArray()
                $images += [pscustomobject]@{ Size = $size; Bytes = $bytes }
                if ($size -eq 256) {
                    [System.IO.File]::WriteAllBytes((Join-Path $assetRoot ($variant.Name + '.png')), $bytes)
                }
            } finally {
                if ($smallGraphics) { $smallGraphics.Dispose() }
                if ($small) { $small.Dispose() }
                $stream.Dispose(); $markBrush.Dispose(); $graphics.Dispose(); $bitmap.Dispose()
            }
        }
        $iconStream = [System.IO.MemoryStream]::new()
        $writer = [System.IO.BinaryWriter]::new($iconStream)
        try {
            $writer.Write([uint16]0); $writer.Write([uint16]1); $writer.Write([uint16]$images.Count)
            $offset = 6 + (16 * $images.Count)
            foreach ($image in $images) {
                $dimension = if ($image.Size -eq 256) { 0 } else { $image.Size }
                $writer.Write([byte]$dimension); $writer.Write([byte]$dimension)
                $writer.Write([byte]0); $writer.Write([byte]0)
                $writer.Write([uint16]1); $writer.Write([uint16]32)
                $writer.Write([uint32]$image.Bytes.Length); $writer.Write([uint32]$offset)
                $offset += $image.Bytes.Length
            }
            foreach ($image in $images) { $writer.Write([byte[]]$image.Bytes) }
            $writer.Flush()
            [System.IO.File]::WriteAllBytes((Join-Path $assetRoot ($variant.Name + '.ico')), $iconStream.ToArray())
        } finally { $writer.Dispose(); $iconStream.Dispose() }
    }
} finally {
    foreach ($path in @($paths) + @($opticalPaths)) { $path.Dispose() }
}
Write-Output 'Transparent Regia icons generated at 16/24/32/48/64/128/256 px for files, Light and Dark windows.'
