# Installs and removes the freshly built MSI. Run only on a disposable Windows
# runner: this exercises Windows Installer, not just archive generation.
param(
    [string]$Target = "x86_64-pc-windows-msvc"
)

$ErrorActionPreference = "Stop"
$package = (Resolve-Path "target/$Target/release/bundle/msi/Token.msi").Path
$output = (New-Item -ItemType Directory -Force "target/verification/windows-msi").FullName
$installDir = Join-Path $output "Token Test Install"

function Invoke-Installer([string]$Operation, [string]$LogName, [string]$Properties = "") {
    $log = Join-Path $output $LogName
    $process = Start-Process msiexec.exe -Wait -PassThru -ArgumentList (
        "$Operation `"$package`" /qn /norestart /l*v `"$log`" $Properties"
    )
    if ($process.ExitCode -ne 0) {
        Get-Content $log -Tail 80
        throw "Windows Installer failed with exit code $($process.ExitCode); see $log"
    }
}

Invoke-Installer "/i" "install.log" "INSTALLDIR=`"$installDir`""
try {
    foreach ($file in @(
        @{ Source = "target/$Target/release/token.exe"; Installed = "token.exe" },
        @{ Source = "Credits.rtf"; Installed = "Resources/Credits.rtf" },
        @{ Source = "LICENSE.md"; Installed = "Resources/LICENSE.md" },
        @{ Source = "assets/Inter_OFL.txt"; Installed = "Resources/assets/Inter_OFL.txt" },
        @{ Source = "assets/OFL.txt"; Installed = "Resources/assets/OFL.txt" }
    )) {
        $installed = Join-Path $installDir $file.Installed
        if ((Get-FileHash $file.Source).Hash -ne (Get-FileHash $installed).Hash) {
            throw "Installed file differs from its source: $installed"
        }
        Write-Host "Verified installed $($file.Installed)"
    }

    # Icon encoding used to abort the entire resource build, leaving version
    # metadata absent as well. Check the installed executable, not a build log.
    $version = (Get-Item (Join-Path $installDir "token.exe")).VersionInfo
    $metadata = cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw "cargo metadata failed" }
    $expectedVersion = ($metadata.packages | Where-Object name -EQ "token").version
    if ($version.ProductName -ne "Token" -or $version.ProductVersion -ne $expectedVersion) {
        throw "Missing or incorrect Windows executable version resources"
    }
    Write-Host "Verified embedded Token $expectedVersion resources"
} finally {
    Invoke-Installer "/x" "uninstall.log"
}

if (Test-Path (Join-Path $installDir "token.exe")) {
    throw "Uninstall left the executable behind"
}
Write-Host "MSI install, payload, resources and uninstall checks passed"
