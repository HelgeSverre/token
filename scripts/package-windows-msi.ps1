# Requires Windows/MSVC and WiX Toolset 3.14 (on PATH or installed under WIX).
param(
    [ValidateSet("x86_64-pc-windows-msvc")]
    [string]$Target = "x86_64-pc-windows-msvc"
)

$ErrorActionPreference = "Stop"
if (-not $IsWindows) { throw "Windows MSI packaging requires Windows" }

$metadata = cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
if ($LASTEXITCODE -ne 0) { throw "cargo metadata failed" }
$package = $metadata.packages | Where-Object name -EQ "token"
$sourceDir = Split-Path $package.manifest_path
$releaseDir = Join-Path $metadata.target_directory "$Target/release"
$output = (New-Item -ItemType Directory -Force "$releaseDir/bundle/msi").FullName

# Cargo reports the exact build-script output directory; do not guess which
# hashed target/build directory contains this build's generated ICO.
$messages = cargo build --release --target $Target --bin token --message-format=json |
    ForEach-Object { $_ | ConvertFrom-Json }
if ($LASTEXITCODE -ne 0) { throw "Windows executable build failed" }
$resourceDir = $messages |
    Where-Object { $_.reason -eq "build-script-executed" -and $_.package_id -eq $package.id } |
    Select-Object -Last 1 -ExpandProperty out_dir
$icon = (Resolve-Path (Join-Path $resourceDir "token.ico")).Path

# The license dialog needs RTF; keep the existing ASCII MIT license as its
# source, escaping RTF control characters rather than maintaining another copy.
$license = (Get-Content (Join-Path $sourceDir "LICENSE.md") -Raw).
    Replace('\', '\\').Replace('{', '\{').Replace('}', '\}').
    Replace("`r", '').Replace("`n", '\par ')
$licensePath = Join-Path $output "License.rtf"
Set-Content $licensePath ('{\rtf1\ansi\deff0{\fonttbl{\f0 Segoe UI;}}\f0\fs20 ' + $license + '}') -NoNewline

$candle = if ($env:WIX) { Join-Path $env:WIX "bin/candle.exe" } else { "candle.exe" }
$light = if ($env:WIX) { Join-Path $env:WIX "bin/light.exe" } else { "light.exe" }
$object = Join-Path $output "Token.wixobj"
& $candle -nologo -arch x64 "-dVersion=$($package.version)" "-dSourceDir=$sourceDir" `
    "-dExecutable=$releaseDir/token.exe" "-dIcon=$icon" "-dLicense=$licensePath" `
    -out $object "$PSScriptRoot/windows-installer.wxs"
if ($LASTEXITCODE -ne 0) { throw "WiX compilation failed" }
& $light -nologo -ext WixUIExtension -cultures:en-us -out "$output/Token.msi" $object
if ($LASTEXITCODE -ne 0) { throw "WiX linking/validation failed" }
Write-Host "Created $output/Token.msi"
