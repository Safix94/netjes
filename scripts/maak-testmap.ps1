<#
.SYNOPSIS
  Maakt een testmap met 1.000 bestanden om Netjes op te proberen.

.DESCRIPTION
  600 kleine, geldige PNG's met variërende kleur (de Windows-shell maakt er miniaturen van,
  een deel heet als screenshot), 100 lege MP4-bestanden en 300 documenten (pdf, docx, xlsx,
  zip, exe-naam, txt), met een handvol exacte duplicaten. Deterministisch: dezelfde oproep
  geeft dezelfde map. Bestaat de map al en is ze niet leeg, dan stopt het script, tenzij
  je -Force meegeeft: dan wordt ze eerst leeggemaakt.

.PARAMETER Pad
  De doelmap. Standaard C:\Users\<jij>\Desktop\Netjes-testmap.

.PARAMETER Force
  Maak een bestaande, gevulde map eerst leeg.

.EXAMPLE
  .\scripts\maak-testmap.ps1
  .\scripts\maak-testmap.ps1 -Force
  .\scripts\maak-testmap.ps1 -Pad D:\tmp\netjes -Force
#>
[CmdletBinding()]
param(
  [string]$Pad = (Join-Path $env:USERPROFILE "Desktop\Netjes-testmap"),
  [switch]$Force
)

$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing

# Het script maakt enkel een map leeg die het zelf gemaakt heeft: die herken je aan het merkteken.
$Merkteken = ".netjes-testmap"

if (Test-Path -LiteralPath $Pad) {
  $bestaand = @(Get-ChildItem -LiteralPath $Pad -Force)
  if ($bestaand.Count -gt 0) {
    if (-not (Test-Path -LiteralPath (Join-Path $Pad $Merkteken))) {
      Write-Error "De map '$Pad' is niet door dit script gemaakt (geen $Merkteken) en wordt niet leeggemaakt. Kies een andere map."
      exit 1
    }
    if (-not $Force) {
      Write-Error "De map '$Pad' bestaat al en is niet leeg. Geef -Force mee om ze eerst leeg te maken."
      exit 1
    }
    Write-Host "Map leegmaken: $Pad"
    Get-ChildItem -LiteralPath $Pad -Force | Remove-Item -Recurse -Force -Confirm:$false
  }
} else {
  New-Item -ItemType Directory -Path $Pad | Out-Null
}
$merk = New-Item -ItemType File -Path (Join-Path $Pad $Merkteken) -Force
$merk.Attributes = $merk.Attributes -bor [System.IO.FileAttributes]::Hidden

$rnd = [System.Random]::new(7)
$gemaakt = 0

function Schrijf-Png {
  param([string]$Bestand, [int]$Zaad)
  $r = [System.Random]::new($Zaad)
  $breedte = 64 + 8 * $r.Next(0, 6)
  $hoogte = 64 + 8 * $r.Next(0, 6)
  $bmp = [System.Drawing.Bitmap]::new($breedte, $hoogte)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  try {
    $achter = [System.Drawing.Color]::FromArgb(255, $r.Next(40, 220), $r.Next(40, 220), $r.Next(40, 220))
    $voor = [System.Drawing.Color]::FromArgb(255, $r.Next(160, 255), $r.Next(160, 255), $r.Next(160, 255))
    $g.Clear($achter)
    $kwast = [System.Drawing.SolidBrush]::new($voor)
    $w = $r.Next(16, [int]([Math]::Min($breedte, $hoogte) * 0.7))
    $g.FillRectangle($kwast, $r.Next(0, $breedte - $w), $r.Next(0, $hoogte - $w), $w, [int]($w * (0.5 + $r.NextDouble())))
    $kwast.Dispose()
    $bmp.Save($Bestand, [System.Drawing.Imaging.ImageFormat]::Png)
  } finally {
    $g.Dispose()
    $bmp.Dispose()
  }
}

function Schrijf-Tekst {
  param([string]$Bestand, [string]$Inhoud)
  [System.IO.File]::WriteAllText($Bestand, $Inhoud, [System.Text.Encoding]::UTF8)
}

# 600 PNG's: 440 foto's, 140 screenshots (Nederlands patroon), 20 Windows-patroon "Screenshot (n)".
$pngNamen = New-Object System.Collections.Generic.List[string]
for ($i = 0; $i -lt 600; $i++) {
  if ($i -lt 440) {
    $naam = "IMG_{0}.png" -f (1000 + $i * 7)
  } elseif ($i -lt 580) {
    $dag = 1 + ($i % 28)
    $naam = "Schermafbeelding 2026-10-{0:d2} {1:d2}{2:d2}{3:d2}.png" -f $dag, (8 + ($i % 12)), ($i % 60), (($i * 7) % 60)
  } else {
    $naam = "Screenshot ({0}).png" -f ($i - 579)
  }
  $bestand = Join-Path $Pad $naam
  Schrijf-Png -Bestand $bestand -Zaad ($i + 100)
  $pngNamen.Add($naam)
  $gemaakt++
}

# 100 lege MP4-namen: 80 gewone video's, 20 schermopnames.
for ($i = 0; $i -lt 100; $i++) {
  $naam = if ($i -lt 80) { "IMG_{0}.mp4" -f (5000 + $i * 3) } else { "Schermopname {0}.mp4" -f ($i - 79) }
  New-Item -ItemType File -Path (Join-Path $Pad $naam) | Out-Null
  $gemaakt++
}

# 300 documenten: 70 pdf, 60 docx, 40 xlsx, 50 zip, 30 exe-naam, 50 txt.
$docs = @(
  @{ n = 70; ext = "pdf";  namen = @("factuur", "offerte", "contract", "handleiding", "ticket") },
  @{ n = 60; ext = "docx"; namen = @("notities", "brief", "verslag", "script") },
  @{ n = 40; ext = "xlsx"; namen = @("begroting", "planning", "uren") },
  @{ n = 50; ext = "zip";  namen = @("archief", "export", "backup") },
  @{ n = 30; ext = "exe";  namen = @("Setup", "Installer", "Update") },
  @{ n = 50; ext = "txt";  namen = @("todo", "idee", "aantekening") }
)
$txtNamen = New-Object System.Collections.Generic.List[string]
foreach ($d in $docs) {
  for ($i = 0; $i -lt $d.n; $i++) {
    $basis = $d.namen[$i % $d.namen.Count]
    $naam = "{0}-{1}.{2}" -f $basis, (1000 + $i * 13 + $d.n), $d.ext
    $bestand = Join-Path $Pad $naam
    switch ($d.ext) {
      "pdf" { Schrijf-Tekst $bestand ("%PDF-1.4`n% testbestand {0} voor Netjes`n%%EOF`n" -f $i) }
      "txt" { Schrijf-Tekst $bestand ("Testbestand {0} voor Netjes. Regel {1}.`n" -f $naam, $rnd.Next(1, 999999)); $txtNamen.Add($naam) }
      default {
        $bytes = New-Object byte[] (64 + $rnd.Next(0, 512))
        $rnd.NextBytes($bytes)
        [System.IO.File]::WriteAllBytes($bestand, $bytes)
      }
    }
    $gemaakt++
  }
}

# Een handvol exacte duplicaten: 6 PNG's en 2 tekstbestanden, als " (1)"-kopie.
$dupes = 0
foreach ($i in 0, 25, 50, 75, 100, 125) {
  $bron = Join-Path $Pad $pngNamen[$i]
  $kopie = Join-Path $Pad ([System.IO.Path]::GetFileNameWithoutExtension($pngNamen[$i]) + " (1).png")
  Copy-Item -LiteralPath $bron -Destination $kopie
  $dupes++
}
foreach ($i in 0, 10) {
  $bron = Join-Path $Pad $txtNamen[$i]
  $kopie = Join-Path $Pad ([System.IO.Path]::GetFileNameWithoutExtension($txtNamen[$i]) + " (1).txt")
  Copy-Item -LiteralPath $bron -Destination $kopie
  $dupes++
}

$totaal = @(Get-ChildItem -LiteralPath $Pad -File).Count
Write-Host ("Klaar: {0} bestanden in {1} ({2} gemaakt, {3} duplicaten erbij)." -f $totaal, $Pad, $gemaakt, $dupes)
