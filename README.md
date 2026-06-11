# DJ Assistant

Le poste de prépa entre Soulseek et les platines. DJ Assistant écoute, vérifie et
range tes téléchargements : il repère les **faux fichiers** (MP3 transcodés vendus
pour du lossless) au spectrogramme, évite les **doublons** et ce qui est **déjà dans
ta biblio**, **convertit au format CDJ** au moment du rangement, **renomme** depuis
Discogs, et pousse tes dossiers en **playlists Rekordbox**. Un seul geste par
morceau : écouter → ranger ou jeter.

> ⚠️ Ce dépôt ne contient pour l'instant que `index.html` — une **maquette
> interactive d'UI/UX**. Pas de vrai audio ni traitement : c'est une démo navigable
> pour présenter le concept. L'app réelle (desktop) reste à construire.

## Fonctions (cible)

- **Écoute & pré-sélection** — waveform cliquable, accélération/ralenti type platine (±%).
- **Détecteur de faux** — spectrogramme + analyse de coupure, sensibilité réglable.
- **Doublons & déjà-en-biblio** — par empreinte audio, indépendamment du nom.
- **Conversion au rangement** — AIFF / WAV / MP3 320, lossy/lossless séparés, pas d'upscale.
- **Nommage & métadonnées Discogs** — pochette, label, année, genre + lien release exacte.
- **Rangement physique en dossiers** — déplacement réel, création à la volée, mode batch.
- **Sync Rekordbox** — dossiers → playlists (XML sûr ou master.db natif avec backup), dédoublonnage.
- **Préparation de clé USB** — formatage FAT32 (tous CDJ) ou exFAT (CDJ récents).

## Lancer la maquette

C'est un fichier statique autonome :

```bash
# ouvrir directement
open index.html        # macOS
start index.html       # Windows

# ou via un serveur local
npx serve .
```

## Déploiement (Vercel)

Aucun build — `index.html` est servi à la racine.

```bash
vercel
```

Framework preset : **Other**.

## Pile technique envisagée (app réelle)

Tauri (Rust + webview), FFmpeg, wavesurfer.js, SoundTouch.js (time-stretch / key-lock),
SQLite + sqlite-vec, Chromaprint/AcoustID (empreinte), API Discogs.
