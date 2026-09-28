# Build & développement — Video DL

## TL;DR

```powershell
.\scripts\dev.ps1            # compile + tests
.\scripts\dev.ps1 build
.\scripts\dev.ps1 build --release
```

Le script met en place l'environnement et lance `cargo`. Utilise-le plutôt que
`cargo` directement, à cause de la contrainte ci-dessous.

## Contrainte : nom d'utilisateur non-ASCII

Le profil Windows est `C:\Users\Rédouane\...` (le `é`). Le toolchain **GNU** de
Rust utilise le linker **MinGW `ld`**, qui lit son fichier de réponse en **ANSI**.
Les chemins UTF-8 contenant `é` sont alors mal décodés et `ld` renvoie
`cannot find ...libstd-*.rlib`, alors que les fichiers existent bel et bien.

**Contournement appliqué** (automatisé par `scripts/dev.ps1`) : garder le
toolchain et la sortie de build sur des chemins **ASCII**.

| Élément | Emplacement ASCII |
|---|---|
| `RUSTUP_HOME` (sysroot du toolchain) | `C:\Users\Public\rustup` |
| `CARGO_TARGET_DIR` (objets, rlibs, exe) | `C:\Users\Public\vdl\target` |
| MinGW (dlltool/gcc/ld) | installé via winget, ajouté au `PATH` par le script |

> Le code source peut rester sous `C:\Users\Rédouane\Desktop\Video DL` : son
> chemin n'est pas passé au linker (seuls les objets/rlibs le sont).

## Installation initiale (machine vierge)

```powershell
# 1. Rust (rustup)
winget install Rustlang.Rustup -e

# 2. Linker MinGW-w64 (POSIX/MSVCRT = ABI du target gnu)
winget install BrechtSanders.WinLibs.POSIX.MSVCRT -e --scope user

# 3. Toolchain GNU installé sur un chemin ASCII
$env:RUSTUP_HOME = 'C:\Users\Public\rustup'
rustup toolchain install stable-x86_64-pc-windows-gnu --profile minimal
```

## Alternative propre : MSVC

Si tu installes **Visual Studio Build Tools** (workload « C++ »,
`winget install Microsoft.VisualStudio.2022.BuildTools` — nécessite l'élévation
UAC), le linker MSVC `link.exe` gère correctement les chemins non-ASCII. Dans ce
cas tout le contournement ci-dessus devient inutile et un simple `cargo test`
fonctionne. C'est l'option recommandée à terme, notamment pour le build **Tauri**
(phase Frontend), où MSVC est le chemin le mieux supporté sous Windows.

## Génération des types TypeScript

Les types TS du contrat IPC sont générés par `ts-rs` **pendant les tests** :
`.\scripts\dev.ps1 test` (ou `cargo test`) écrit les `.ts` dans
`apps/frontend/src/types/`. Ne pas les éditer à la main.
