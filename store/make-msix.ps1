# Wraps a finished Windows build (a folder with the app's .exe in it) into an .msix for the
# Microsoft Store. The package is left unsigned on purpose: the Store signs it with Microsoft's
# certificate when it's published, which is what makes the SmartScreen warning go away for free.
#
# Identity values come from Partner Center > the app > Product management > Product identity.
# DisplayName must match a name reserved for that app, character for character.
#
#   pwsh store/make-msix.ps1 -AppDir dist\TallyReel -Exe TallyReel.exe -Name Soonbuilt.TallyReel `
#        -DisplayName TallyReel -Version 1.0.15 -Logo packaging\icon-1024.png -Out dist\TallyReel.msix
param(
  [Parameter(Mandatory)] [string] $AppDir,
  [Parameter(Mandatory)] [string] $Exe,
  [Parameter(Mandatory)] [string] $Name,
  [Parameter(Mandatory)] [string] $DisplayName,
  [Parameter(Mandatory)] [string] $Description,
  [Parameter(Mandatory)] [string] $Version,
  [Parameter(Mandatory)] [string] $Logo,
  [Parameter(Mandatory)] [string] $Out,
  [string] $Publisher = "CN=4F95B1D3-7C72-45FD-B662-2EA9E1B20762",
  [string] $PublisherDisplayName = "Soonbuilt",
  [switch] $Microphone
)
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing

# The Store wants four numbers with the last one 0: 1.0.15 -> 1.0.15.0
$parts = @($Version -split '[^0-9]+' | Where-Object { $_ -ne '' } | Select-Object -First 3)
while ($parts.Count -lt 3) { $parts += '0' }
$msixVersion = ($parts + '0') -join '.'

$stage = Join-Path ([IO.Path]::GetTempPath()) ("msix-" + [guid]::NewGuid())
Copy-Item -Recurse $AppDir $stage
if (-not (Test-Path (Join-Path $stage $Exe))) { throw "$Exe isn't in $AppDir" }

# Tile and taskbar images, cut from one square source image. Plain names at their base size, so
# no resources.pri is needed to find them.
$assets = New-Item -ItemType Directory (Join-Path $stage "Assets")
$src = [Drawing.Image]::FromFile((Resolve-Path $Logo))
foreach ($a in @(@{ n = "Square44x44Logo"; s = 44 }, @{ n = "Square150x150Logo"; s = 150 }, @{ n = "StoreLogo"; s = 50 })) {
  $bmp = New-Object Drawing.Bitmap $a.s, $a.s
  $g = [Drawing.Graphics]::FromImage($bmp)
  $g.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
  $g.DrawImage($src, 0, 0, $a.s, $a.s)
  $bmp.Save((Join-Path $assets "$($a.n).png"), [Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose(); $bmp.Dispose()
}
$src.Dispose()

$esc = { param($s) [Security.SecurityElement]::Escape($s) }
$mic = if ($Microphone) { '<DeviceCapability Name="microphone" />' } else { '' }
@"
<?xml version="1.0" encoding="utf-8"?>
<Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
         xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
         xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"
         IgnorableNamespaces="uap rescap">
  <Identity Name="$(& $esc $Name)" Publisher="$(& $esc $Publisher)" Version="$msixVersion" ProcessorArchitecture="x64" />
  <Properties>
    <DisplayName>$(& $esc $DisplayName)</DisplayName>
    <PublisherDisplayName>$(& $esc $PublisherDisplayName)</PublisherDisplayName>
    <Logo>Assets\StoreLogo.png</Logo>
  </Properties>
  <Dependencies>
    <TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.17763.0" MaxVersionTested="10.0.26100.0" />
  </Dependencies>
  <Resources>
    <Resource Language="en-us" />
  </Resources>
  <Applications>
    <Application Id="App" Executable="$(& $esc $Exe)" EntryPoint="Windows.FullTrustApplication">
      <uap:VisualElements DisplayName="$(& $esc $DisplayName)" Description="$(& $esc $Description)"
                          BackgroundColor="transparent"
                          Square150x150Logo="Assets\Square150x150Logo.png"
                          Square44x44Logo="Assets\Square44x44Logo.png" />
    </Application>
  </Applications>
  <Capabilities>
    <rescap:Capability Name="runFullTrust" />
    $mic
  </Capabilities>
</Package>
"@ | Set-Content -Encoding utf8 (Join-Path $stage "AppxManifest.xml")

# makeappx ships with the Windows SDK on GitHub's Windows machines; take the newest.
$makeappx = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\makeappx.exe" |
  Sort-Object FullName -Descending | Select-Object -First 1
if (-not $makeappx) { throw "makeappx.exe not found (Windows SDK missing)" }

$outPath = [IO.Path]::GetFullPath($Out)
New-Item -ItemType Directory -Force (Split-Path $outPath) | Out-Null
& $makeappx.FullName pack /d $stage /p $outPath /o
if ($LASTEXITCODE -ne 0) { throw "makeappx failed ($LASTEXITCODE)" }
Remove-Item -Recurse -Force $stage
Write-Host "Built $outPath ($Name $msixVersion)"
