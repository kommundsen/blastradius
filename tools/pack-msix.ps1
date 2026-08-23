# One-command Store pack (spec/msix-store-packaging.md): build fresh, stage
# both exes, verify the binaries actually carry the manifest's version, pack.
#
#   .\tools\pack-msix.ps1                # x64, unsigned, for Store upload
#   .\tools\pack-msix.ps1 -Arch arm64    # arm64 cross-build (needs rustup
#                                        #   target aarch64-pc-windows-msvc)
#   .\tools\pack-msix.ps1 -DevCert       # dev-signed, for local Add-AppxPackage
#
# Exists because of the 0.2.0.0 incident (2026-08-23): packing from a stale
# target\release\ exe shipped 0.1.0 binaries under a 0.2.0.0 label. This
# script makes the rebuild non-optional and refuses a version mismatch.
param(
  [switch]$DevCert,
  [ValidateSet('x64', 'arm64')][string]$Arch = 'x64'
)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root

# manifest version is the source of truth for the release
$manifest = Get-Content packaging\msix\Package.appxmanifest -Raw
if ($manifest -notmatch 'Version="(\d+\.\d+\.\d+)\.0"') { throw 'manifest version not found' }
$version = $Matches[1]

# the three surfaces must agree before anything is built
$cargo = Get-Content Cargo.toml -Raw
$tauri = Get-Content crates\blastradius-app\tauri.conf.json -Raw
if ($cargo -notmatch [regex]::Escape("version = `"$version`"")) { throw "Cargo.toml is not at $version" }
if ($tauri -notmatch [regex]::Escape("`"version`": `"$version`"")) { throw "tauri.conf.json is not at $version" }
Write-Host "version: $version.0 (manifest, Cargo.toml, tauri.conf.json agree) [$Arch]"

# fresh release build — never pack whatever happens to be in target\release
if ($Arch -eq 'arm64') {
  cargo build --release --target aarch64-pc-windows-msvc -p blastradius-app -p blastradius-cli
  $bin = 'target\aarch64-pc-windows-msvc\release'
} else {
  cargo build --release -p blastradius-app -p blastradius-cli
  $bin = 'target\release'
}
if ($LASTEXITCODE -ne 0) { throw 'release build failed' }

# stage exactly the payload
New-Item -ItemType Directory -Force packaging\msix\dist | Out-Null
Remove-Item packaging\msix\dist\* -Force
Copy-Item $bin\blastradius-app.exe, $bin\blastradius.exe packaging\msix\dist\

# the built exe must self-report the manifest's version
$fv = (Get-Item packaging\msix\dist\blastradius-app.exe).VersionInfo.ProductVersion
if ($fv -ne $version) { throw "blastradius-app.exe reports $fv, manifest says $version — stale build?" }

Set-Location packaging\msix
if ($DevCert) {
  winapp cert generate --if-exists skip
  winapp pack ./dist --cert ./devcert.pfx
} else {
  winapp pack ./dist
}
if ($LASTEXITCODE -ne 0) { throw 'winapp pack failed' }
Write-Host "`nStore-ready: packaging\msix\25829KimOmmundsen.Blastradius_$version.0_$Arch.msix"
