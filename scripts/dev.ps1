# Dev build/test wrapper for Video DL.
#
# Why this exists: the Windows profile path contains a non-ASCII character
# (e.g. non-ASCII characters in the user profile path). The MinGW linker (ld) reads its linker-response file
# as ANSI, so UTF-8 paths with "é" get mangled and ld "cannot find" the .rlib /
# .o files. Fix: keep the Rust toolchain (sysroot) and the build output on
# ASCII-only paths under C:\Users\Public, and put MinGW on PATH.
#
# Usage:
#   .\scripts\dev.ps1            # runs `cargo test`
#   .\scripts\dev.ps1 build
#   .\scripts\dev.ps1 build --release
#   .\scripts\dev.ps1 clippy
#
# One-time setup (if a fresh machine):
#   winget install BrechtSanders.WinLibs.POSIX.MSVCRT -e --scope user
#   $env:RUSTUP_HOME='C:\Users\Public\rustup'
#   rustup toolchain install stable-x86_64-pc-windows-gnu --profile minimal
#
# Note: we do NOT set $ErrorActionPreference='Stop' — under Windows PowerShell
# 5.1 that turns cargo's normal stderr output into a terminating error. The two
# pre-flight `throw`s below still abort on their own.

$RustupHome = 'C:\Users\Public\rustup'
$CargoHome  = 'C:\Users\Public\cargo'
$TargetDir  = 'C:\Users\Public\vdl\target'
$Toolchain  = 'stable-x86_64-pc-windows-gnu'
$CargoBin   = Join-Path $env:USERPROFILE '.cargo\bin'

# Locate the WinLibs MinGW bin dir (provides dlltool.exe, gcc.exe, ld.exe).
$mingw = Get-ChildItem "$env:LOCALAPPDATA\Microsoft\WinGet\Packages" -Recurse -Filter 'dlltool.exe' -ErrorAction SilentlyContinue |
    Select-Object -First 1 -ExpandProperty DirectoryName
if (-not $mingw) {
    throw "MinGW (dlltool.exe) not found. Install it with: winget install BrechtSanders.WinLibs.POSIX.MSVCRT -e --scope user"
}
if (-not (Test-Path "$RustupHome\toolchains\$Toolchain")) {
    throw "GNU toolchain missing at $RustupHome. Install with: `$env:RUSTUP_HOME='$RustupHome'; rustup toolchain install $Toolchain --profile minimal"
}

$env:Path             = "$CargoBin;$mingw;$env:Path"
$env:RUSTUP_HOME      = $RustupHome
$env:CARGO_HOME       = $CargoHome      # registry/cache on an ASCII path too
$env:CARGO_TARGET_DIR = $TargetDir

$crateDir = Join-Path $PSScriptRoot '..\apps\src-tauri'
Push-Location $crateDir
try {
    # @(...) forces an array: a single-element array would otherwise unwrap to a
    # String, and splatting a String (@cargoArgs) explodes it into characters.
    $cargoArgs = @(if ($args.Count -eq 0) { 'test' } else { $args })
    & cargo "+$Toolchain" @cargoArgs
    exit $LASTEXITCODE
} finally {
    Pop-Location
}
