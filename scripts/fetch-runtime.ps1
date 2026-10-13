# Télécharge les binaires runtime (yt-dlp, FFmpeg, Deno) dans apps/src-tauri/binaries/.
#
# Ces binaires sont volumineux (FFmpeg > 100 Mo = au-dessus de la limite GitHub)
# et ne sont donc PAS versionnés. Lance ce script une fois après un `git clone`,
# avant de builder l'app, pour récupérer les ressources que l'installeur embarque.
#
#   .\scripts\fetch-runtime.ps1          # télécharge ce qui manque
#   .\scripts\fetch-runtime.ps1 -Force   # retélécharge tout

param([switch]$Force)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$bin = Join-Path $PSScriptRoot '..\apps\src-tauri\binaries'
New-Item -ItemType Directory -Force $bin | Out-Null
$tmp = Join-Path $env:TEMP ("vdl-runtime-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force $tmp | Out-Null

function Need($name) {
    $p = Join-Path $bin $name
    if ($Force -or -not (Test-Path $p)) { return $true }
    Write-Host "  déjà présent : $name"; return $false
}

try {
    # 1) yt-dlp (exe autonome, pas besoin de Python). Nom attendu par le code
    #    et par tauri.conf.json : yt-dlp-x86_64-pc-windows-msvc.exe
    if (Need 'yt-dlp-x86_64-pc-windows-msvc.exe') {
        Write-Host "Téléchargement yt-dlp..."
        Invoke-WebRequest 'https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe' `
            -OutFile (Join-Path $bin 'yt-dlp-x86_64-pc-windows-msvc.exe')
    }

    # 2) Deno (runtime JS pour la signature YouTube).
    if (Need 'deno.exe') {
        Write-Host "Téléchargement Deno..."
        $z = Join-Path $tmp 'deno.zip'
        Invoke-WebRequest 'https://github.com/denoland/deno/releases/latest/download/deno-x86_64-pc-windows-msvc.zip' -OutFile $z
        Expand-Archive $z -DestinationPath (Join-Path $tmp 'deno') -Force
        Copy-Item (Join-Path $tmp 'deno\deno.exe') (Join-Path $bin 'deno.exe') -Force
    }

    # 3) FFmpeg (build LGPL statique : décodeurs dont AAC + https, sans les
    #    encodeurs GPL inutiles ici puisqu'on remuxe en copie).
    if (Need 'ffmpeg.exe') {
        Write-Host "Téléchargement FFmpeg (LGPL statique, ~130 Mo)..."
        $z = Join-Path $tmp 'ffmpeg.zip'
        Invoke-WebRequest 'https://github.com/BtbN/FFmpeg-Builds/releases/latest/download/ffmpeg-master-latest-win64-lgpl.zip' -OutFile $z
        Expand-Archive $z -DestinationPath (Join-Path $tmp 'ff') -Force
        $exe = Get-ChildItem (Join-Path $tmp 'ff') -Recurse -Filter ffmpeg.exe | Select-Object -First 1
        Copy-Item $exe.FullName (Join-Path $bin 'ffmpeg.exe') -Force
    }

    Write-Host ""
    Write-Host "Binaires runtime prêts dans $bin :"
    Get-ChildItem $bin -Filter *.exe | ForEach-Object { "  {0,-42} {1,7:N1} Mo" -f $_.Name, ($_.Length / 1MB) }
}
finally {
    Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
}
