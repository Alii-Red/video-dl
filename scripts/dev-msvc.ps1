# MSVC build/run wrapper for Video DL (Tauri desktop app).
#
# Unlike dev.ps1 (which uses the GNU toolchain + MinGW as an MSVC-less
# fallback), this uses the default MSVC toolchain. MSVC's link.exe handles the
# non-ASCII profile path (e.g. accented characters) fine, so the only reason we
# still relocate CARGO_HOME / the target dir onto C:\Users\Public is to reuse
# the crate cache from earlier builds and keep the msvc/gnu build outputs apart.
#
# Usage:
#   .\scripts\dev-msvc.ps1                 # cargo build
#   .\scripts\dev-msvc.ps1 run             # cargo run (launches the app)
#   .\scripts\dev-msvc.ps1 test --no-run

$CargoHome = 'C:\Users\Public\cargo'
$TargetDir = 'C:\Users\Public\vdl\target-msvc'
$Toolchain = 'stable-x86_64-pc-windows-msvc'
$CargoBin  = Join-Path $env:USERPROFILE '.cargo\bin'

$env:Path             = "$CargoBin;$env:Path"
$env:CARGO_HOME       = $CargoHome
$env:CARGO_TARGET_DIR = $TargetDir

$crateDir = Join-Path $PSScriptRoot '..\apps\src-tauri'
Push-Location $crateDir
try {
    $cargoArgs = @(if ($args.Count -eq 0) { 'build' } else { $args })
    & cargo "+$Toolchain" @cargoArgs
    exit $LASTEXITCODE
} finally {
    Pop-Location
}
