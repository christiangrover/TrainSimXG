# Builds a release and packages it as dist\TableTopTrains-windows-x64.zip.
# Unzip anywhere on a Windows 10/11 PC and run TableTopTrains.exe.
Set-Location $PSScriptRoot
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"

# Link the C runtime statically so the .exe runs on a fresh Windows install
# without the Visual C++ Redistributable. (Only for release packaging: dev
# builds keep the default so their build tools aren't all rebuilt.)
$env:RUSTFLAGS = "-C target-feature=+crt-static"

# cargo prints progress on stderr; only its exit code signals failure.
cargo build --release
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
$ErrorActionPreference = "Stop"

$stage = "dist\TableTopTrains"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force $stage | Out-Null
Copy-Item target\release\trainsim.exe "$stage\TableTopTrains.exe"
Copy-Item README.md $stage

$zip = "dist\TableTopTrains-windows-x64.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path $stage -DestinationPath $zip
Write-Host "Packaged $zip"
