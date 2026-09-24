# Generate WeChatBridge brand icons: green circle + white "wei" glyph (U+5FAE)
Add-Type -AssemblyName System.Drawing

$green = [System.Drawing.Color]::FromArgb(7, 193, 96)
$white = [System.Drawing.Color]::White
$iconsDir = "e:\WorkBuddy-work\trae\WeChatBridge\src-tauri\icons"
$glyph = [char]0x5FAE  # the Chinese character for the ball

function New-BallIcon([int]$size, [string]$path) {
    $bmp = New-Object System.Drawing.Bitmap($size, $size)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::AntiAliasGridFit
    $g.Clear([System.Drawing.Color]::Transparent)

    $brush = New-Object System.Drawing.SolidBrush($green)
    $g.FillEllipse($brush, 0, 0, $size, $size)

    $highlight = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(40, 255, 255, 255))
    $g.FillEllipse($highlight, [int]($size * 0.12), [int]($size * 0.06), [int]($size * 0.76), [int]($size * 0.42))

    $fontSize = [float]($size * 0.46)
    $font = New-Object System.Drawing.Font("Microsoft YaHei", $fontSize, [System.Drawing.FontStyle]::Bold, [System.Drawing.GraphicsUnit]::Pixel)
    $textBrush = New-Object System.Drawing.SolidBrush($white)
    $sf = New-Object System.Drawing.StringFormat
    $sf.Alignment = [System.Drawing.StringAlignment]::Center
    $sf.LineAlignment = [System.Drawing.StringAlignment]::Center
    $rect = New-Object System.Drawing.RectangleF(0, 0, $size, $size)
    $g.DrawString([string]$glyph, $font, $textBrush, $rect, $sf)

    $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
}

New-BallIcon 32 "$iconsDir\32x32.png"
New-BallIcon 128 "$iconsDir\128x128.png"
New-BallIcon 256 "$iconsDir\128x128@2x.png"
New-BallIcon 512 "$iconsDir\icon.png"

# ICO with PNG-compressed entries (16/32/48/256)
$pngStreams = @()
foreach ($s in @(16, 32, 48, 256)) {
    $tmp = Join-Path $env:TEMP "wcb_icon_$s.png"
    New-BallIcon $s $tmp
    $pngStreams += ,([System.IO.File]::ReadAllBytes($tmp))
}

$icoPath = "$iconsDir\icon.ico"
$fs = [System.IO.File]::Create($icoPath)
$bw = New-Object System.IO.BinaryWriter($fs)
$bw.Write([uint16]0)
$bw.Write([uint16]1)
$bw.Write([uint16]$pngStreams.Count)
$offset = 6 + 16 * $pngStreams.Count
foreach ($i in 0..($pngStreams.Count - 1)) {
    $size = @(16, 32, 48, 256)[$i]
    $data = $pngStreams[$i]
    if ($size -ge 256) { $wh = [byte]0 } else { $wh = [byte]$size }
    $bw.Write($wh)
    $bw.Write($wh)
    $bw.Write([byte]0)
    $bw.Write([byte]0)
    $bw.Write([uint16]1)
    $bw.Write([uint16]32)
    $bw.Write([uint32]$data.Length)
    $bw.Write([uint32]$offset)
    $offset += $data.Length
}
foreach ($data in $pngStreams) { $bw.Write($data) }
$bw.Close(); $fs.Close()

Get-Item "$iconsDir\32x32.png","$iconsDir\128x128.png","$iconsDir\128x128@2x.png","$iconsDir\icon.png","$iconsDir\icon.ico" |
  Select-Object Name, Length
