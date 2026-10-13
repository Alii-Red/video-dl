# Installation pas à pas (pour débutant complet)

Ce guide explique **tout**, en partant de zéro, pour installer et lancer
Video DL sur un PC **Windows 10 ou 11**. Aucune connaissance technique n'est
supposée : il suffit de suivre les étapes **dans l'ordre** et de copier-coller
les commandes.

> ⏱️ Comptez environ **30 à 60 minutes** la première fois (surtout des
> téléchargements et des installations qui tournent toutes seules).

---

## Comment taper les commandes de ce guide

Plusieurs étapes demandent de coller une commande dans un **terminal**.

1. Appuyez sur la touche **Windows**, tapez `powershell`.
2. Cliquez sur **Windows PowerShell**.
3. Une fenêtre bleue (ou noire) s'ouvre. C'est là qu'on colle les commandes.
4. Pour coller : **clic droit** dans la fenêtre (ou `Ctrl + V`), puis **Entrée**.

Laissez chaque commande se terminer avant de passer à la suivante.

---

## Étape 1 — Installer les outils de base

Il faut installer **5 logiciels**. Pour chacun : téléchargez, lancez le
programme d'installation, et cliquez **Suivant / Installer** jusqu'au bout
(les options par défaut conviennent).

| # | Logiciel | Où le télécharger | À quoi ça sert |
|---|----------|-------------------|----------------|
| 1 | **Git** | https://git-scm.com/download/win | Récupérer le code du projet |
| 2 | **Node.js** (version LTS) | https://nodejs.org | Construire l'interface + faire marcher YouTube |
| 3 | **Python** (3.x) | https://www.python.org/downloads/ | Faire tourner yt-dlp |
| 4 | **Rust** | https://www.rust-lang.org/tools/install | Compiler l'application |
| 5 | **Outils C++ de Visual Studio** | https://visualstudio.microsoft.com/visual-cpp-build-tools/ | Nécessaire pour compiler sous Windows |

### Détails importants pour 2 d'entre eux

**Python (n°3)** : sur le **premier écran** de l'installateur, cochez bien la
case **« Add python.exe to PATH »** (en bas), **puis** cliquez sur *Install Now*.

**Outils C++ (n°5)** : lancez le fichier téléchargé. Dans la fenêtre qui liste
des « charges de travail », cochez **« Développement Desktop en C++ »**
(*Desktop development with C++*), puis cliquez **Installer**. C'est le plus long
(plusieurs Go).

> **WebView2** : normalement déjà présent sur Windows 10/11 (via Edge). Si l'app
> refuse de démarrer plus tard, installez-le ici :
> https://developer.microsoft.com/microsoft-edge/webview2/ (choisir
> *Evergreen Standalone Installer*).

### ⚠️ Après l'étape 1 : redémarrez le PC

Cela garantit que Windows « voit » bien tous les outils installés.

---

## Étape 2 — Vérifier que tout est installé

Ouvrez un **nouveau** PowerShell (voir plus haut) et collez ces commandes
**une par une**. Chacune doit afficher un numéro de version (peu importe lequel).

```powershell
git --version
node --version
python --version
cargo --version
```

Si l'une dit *« n'est pas reconnu… »*, c'est que le logiciel correspondant n'est
pas installé (ou que vous n'avez pas redémarré). Reprenez l'étape 1 pour celui-là.

---

## Étape 3 — Installer yt-dlp et l'outil de build Tauri

Collez ces deux commandes, une par une (elles durent 1 à 2 minutes chacune) :

```powershell
python -m pip install --upgrade --pre "yt-dlp[default]"
cargo install tauri-cli --version "^2"
```

---

## Étape 4 — (Recommandé) un FFmpeg complet

Video DL a besoin de **FFmpeg** pour assembler l'image et le son. Un FFmpeg
« complet » est **détecté automatiquement** s'il est déjà sur le PC.

Le plus simple : installez **Shutter Encoder** (gratuit) depuis
https://www.shutterencoder.com — l'app s'en servira toute seule.

> Si vous sautez cette étape, YouTube fonctionnera, mais certains sites en
> « streaming » (HLS) risquent d'échouer au dernier moment. Une bannière dans
> les **Réglages** de l'app vous préviendra le cas échéant.

---

## Étape 5 — Récupérer le projet

Dans PowerShell, collez (cela crée un dossier `video-dl` sur votre Bureau) :

```powershell
cd $HOME\Desktop
git clone https://github.com/Alii-Red/video-dl.git
cd video-dl
```

> Le dépôt est **privé** : Git vous demandera de vous connecter à GitHub
> (une fenêtre s'ouvre). Connectez-vous avec le compte **Alii-Red**.

---

## Étape 6 — Construire l'application

Toujours dans PowerShell, **dans le dossier `video-dl`**, collez ces commandes
une par une :

```powershell
cd apps\frontend
npm install
cd ..\..
.\scripts\dev-msvc.ps1 tauri build
```

La dernière commande compile tout. **La première fois, c'est long** (plusieurs
minutes) et il y a beaucoup de texte : c'est normal. Attendez la fin.

---

## Étape 7 — Installer l'application

Le build a créé un **installateur**. Lancez-le (double-clic), puis suivez les
étapes :

```
video-dl\...\target-msvc\release\bundle\nsis\
```

> Le chemin exact s'affiche à la fin du build (ligne *« Finished … bundles at »*).
> Le fichier s'appelle `Video DL_..._x64-setup.exe`.

Après installation, lancez **Video DL** depuis le **menu Démarrer**.

> ⚠️ Lancez toujours l'app via le **raccourci du menu Démarrer**, jamais un
> `.exe` trouvé dans un dossier `debug` (sinon la fenêtre serait noire).

---

## Étape 8 — Installer l'extension du navigateur (Opera GX)

Pour que l'app détecte les vidéos automatiquement pendant que vous naviguez :

1. Dans **Opera GX**, ouvrez l'adresse : `opera://extensions`
2. En haut à droite, activez le **Mode développeur**.
3. Cliquez **« Charger une extension non empaquetée »**.
4. Choisissez le dossier : `video-dl\apps\extension`
5. Dans l'app **Video DL**, allez dans **Réglages** : copiez le **port** et le
   **token** affichés.
6. Dans `opera://extensions`, ouvrez les **options** de l'extension Video DL et
   collez-y le port et le token.

C'est prêt ! Naviguez sur un site vidéo : les téléchargements proposés
apparaissent dans la fenêtre Video DL.

> Après toute mise à jour de l'extension, revenez sur `opera://extensions` et
> cliquez le bouton **↻** (recharger) de l'extension Video DL.

---

## En cas de problème

- **Une commande dit « n'est pas reconnu »** → le logiciel correspondant n'est
  pas installé ou le PC n'a pas été redémarré (étape 1 + redémarrage).
- **Le build échoue sur une erreur de « linker » / `link.exe` introuvable** →
  les *Outils C++ de Visual Studio* (étape 1, n°5) ne sont pas installés, ou la
  charge « Développement Desktop en C++ » n'a pas été cochée.
- **Fenêtre noire au lancement** → vous avez lancé le mauvais exe : utilisez le
  raccourci du **menu Démarrer**.
- **YouTube affiche « Video unavailable » / erreur 403** → mettez yt-dlp à jour
  (bouton **« Mettre à jour yt-dlp »** dans Réglages → Maintenance).
- **Un téléchargement HLS échoue à la toute fin** → installez un FFmpeg complet
  (étape 4).

---

## Mises à jour plus tard

Pour récupérer les dernières modifications et reconstruire :

```powershell
cd $HOME\Desktop\video-dl
git pull
cd apps\frontend; npm install; cd ..\..
.\scripts\dev-msvc.ps1 tauri build
```

Puis relancez l'installateur (étape 7).
