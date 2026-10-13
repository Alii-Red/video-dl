# Installation pas à pas (pour débutant complet)

Ce guide explique **tout**, en partant de zéro, pour installer et lancer
Video DL sur un PC **Windows 10 ou 11**. Aucune connaissance technique n'est
supposée : il suffit de suivre les étapes **dans l'ordre** et de copier-coller
les commandes.

Il y a **deux façons** d'installer :

- **Option A — l'installeur tout prêt** (recommandé, 5 minutes) : un seul fichier
  à télécharger, **rien d'autre à installer**. Pour tout le monde.
- **Option B — compiler soi-même** depuis le code source : réservé si vous voulez
  modifier le projet. Plus long, plus technique.

---

# Option A — Installeur tout prêt (recommandé)

L'installeur contient **déjà tout** ce dont l'app a besoin (yt-dlp, FFmpeg, le
moteur YouTube). Vous n'avez **ni Python, ni Node, ni Rust, ni FFmpeg** à
installer.

1. **Téléchargez l'installeur** :
   👉 **https://github.com/Alii-Red/video-dl/releases/latest**
   (connectez-vous au compte **Alii-Red** — le dépôt est privé).
   Sur cette page, dépliez **Assets** (en bas) et cliquez sur
   **`Video DL_..._x64-setup.exe`** pour le télécharger.
2. Lancez-le (double-clic). Si Windows affiche un avertissement
   « Windows a protégé votre ordinateur » → *Informations complémentaires* →
   *Exécuter quand même* (c'est normal pour une app non signée).
   > Si le moteur d'affichage **WebView2** manque, l'installeur le propose
   > automatiquement : laissez-le s'installer.
5. Lancez **Video DL** depuis le **menu Démarrer**.

Puis configurez l'extension du navigateur : voir **[Étape 8](#étape-8--installer-lextension-du-navigateur-opera-gx)** plus bas. C'est tout !

> Mise à jour plus tard : retéléchargez simplement le nouvel installeur depuis
> les Releases et relancez-le. (yt-dlp, lui, se met à jour tout seul via le
> bouton *Réglages → Maintenance*.)

---

# Option B — Compiler soi-même

> ⏱️ Comptez environ **30 à 60 minutes** la première fois (surtout des
> téléchargements et des installations qui tournent toutes seules).

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
| 2 | **Node.js** (version LTS) | https://nodejs.org | Construire l'interface |
| 3 | **Rust** | https://www.rust-lang.org/tools/install | Compiler l'application |
| 4 | **Outils C++ de Visual Studio** | https://visualstudio.microsoft.com/visual-cpp-build-tools/ | Nécessaire pour compiler sous Windows |

> Pas besoin de Python ni de FFmpeg : yt-dlp, FFmpeg et le moteur YouTube sont
> téléchargés automatiquement par un script (étape 5) et embarqués dans
> l'installeur que vous produirez.

### Détail important pour les Outils C++

**Outils C++ (n°4)** : lancez le fichier téléchargé. Dans la fenêtre qui liste
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
cargo --version
```

Si l'une dit *« n'est pas reconnu… »*, c'est que le logiciel correspondant n'est
pas installé (ou que vous n'avez pas redémarré). Reprenez l'étape 1 pour celui-là.

---

## Étape 3 — Installer l'outil de build Tauri

```powershell
cargo install tauri-cli --version "^2"
```

---

## Étape 4 — Récupérer le projet

Dans PowerShell, collez (cela crée un dossier `video-dl` sur votre Bureau) :

```powershell
cd $HOME\Desktop
git clone https://github.com/Alii-Red/video-dl.git
cd video-dl
```

> Le dépôt est **privé** : Git vous demandera de vous connecter à GitHub
> (une fenêtre s'ouvre). Connectez-vous avec le compte **Alii-Red**.

---

## Étape 5 — Télécharger les binaires runtime

yt-dlp, FFmpeg et Deno ne sont pas dans le dépôt (trop volumineux). Un script
les récupère automatiquement (~250 Mo, quelques minutes) :

```powershell
.\scripts\fetch-runtime.ps1
```

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

## Étape 8 — Installer l'extension du navigateur

Pour que l'app détecte les vidéos automatiquement pendant que vous naviguez.
**Le token se configure tout seul** — rien à copier.

1. Dans l'app **Video DL** → **Réglages** → section *Détection live* → cliquez
   **« Ouvrir le dossier de l'extension »** (garde la fenêtre de l'Explorateur
   ouverte).
2. Dans votre navigateur (Opera GX, Chrome, Edge…), ouvrez la page des
   extensions : `opera://extensions` (ou `chrome://extensions`).
3. En haut à droite, activez le **Mode développeur**.
4. Cliquez **« Charger une extension non empaquetée »** et sélectionnez le
   dossier ouvert à l'étape 1.

C'est prêt ! L'extension récupère automatiquement le port et le token de l'app.
Naviguez sur un site vidéo : les téléchargements proposés apparaissent dans la
fenêtre Video DL. (Le point à côté de « Video DL » passe au **vert** quand
l'extension est connectée.)

> Après toute mise à jour de l'extension, revenez sur la page des extensions et
> cliquez le bouton **↻** (recharger).

---

## En cas de problème

- **Une commande dit « n'est pas reconnu »** → le logiciel correspondant n'est
  pas installé ou le PC n'a pas été redémarré (étape 1 + redémarrage).
- **Le build échoue sur une erreur de « linker » / `link.exe` introuvable** →
  les *Outils C++ de Visual Studio* (étape 1, n°4) ne sont pas installés, ou la
  charge « Développement Desktop en C++ » n'a pas été cochée.
- **Fenêtre noire au lancement** → vous avez lancé le mauvais exe : utilisez le
  raccourci du **menu Démarrer**.
- **YouTube affiche « Video unavailable » / erreur 403** → mettez yt-dlp à jour
  (bouton **« Mettre à jour yt-dlp »** dans Réglages → Maintenance).

---

## Mises à jour plus tard (Option B)

Pour récupérer les dernières modifications et reconstruire :

```powershell
cd $HOME\Desktop\video-dl
git pull
.\scripts\fetch-runtime.ps1
cd apps\frontend; npm install; cd ..\..
.\scripts\dev-msvc.ps1 tauri build
```

Puis relancez l'installateur (étape 7).
