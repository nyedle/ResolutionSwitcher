param(
    [ValidateSet('x64', 'arm64')][string]$Arch = 'x64',
    [string]$Version = '0.0.0'
)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root

$target = if ($Arch -eq 'arm64') { 'aarch64-pc-windows-msvc' } else { 'x86_64-pc-windows-msvc' }
$exe = "target\$target\release\resolution-switcher.exe"

rustup target add $target | Out-Null
cargo build --release --target $target
if ($LASTEXITCODE -ne 0) { throw "cargo build failed for $target" }

New-Item -ItemType Directory -Force "dist\$Arch" | Out-Null
Copy-Item $exe "dist\$Arch\" -Force
Copy-Item $exe "dist\resolution-switcher-$Arch.exe" -Force
Write-Host "bare exe  -> dist\resolution-switcher-$Arch.exe"

$stage = "dist\portable-$Arch"
Remove-Item $stage -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $stage | Out-Null
Copy-Item $exe $stage\
Copy-Item installer\portable-readme.txt "$stage\README.txt"
Copy-Item LICENSE "$stage\LICENSE.txt"
'Delete this file to store settings in %APPDATA% instead.' | Out-File -Encoding utf8 "$stage\portable.txt"
Compress-Archive -Path "$stage\*" -DestinationPath "dist\ResolutionSwitcher-$Arch-portable.zip" -Force
Write-Host "portable  -> dist\ResolutionSwitcher-$Arch-portable.zip"

$iscc = (Get-Command iscc -ErrorAction SilentlyContinue).Source
if (-not $iscc) {
    $iscc = @(
        "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe"
        "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe"
        "$env:ProgramFiles\Inno Setup 6\ISCC.exe"
    ) | Where-Object { Test-Path $_ } | Select-Object -First 1
}

if ($iscc) {
    & $iscc /DArch=$Arch /DAppVersion=$Version installer\setup.iss | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "iscc failed for $Arch" }
    Write-Host "installer -> dist\ResolutionSwitcher-Setup-$Arch.exe"
} elseif ($env:CI) {
    throw "Inno Setup not found and this is CI"
} else {
    Write-Host "installer -> skipped, install Inno Setup 6 to build it"
}

Remove-Item $stage, "dist\$Arch" -Recurse -Force -ErrorAction SilentlyContinue
