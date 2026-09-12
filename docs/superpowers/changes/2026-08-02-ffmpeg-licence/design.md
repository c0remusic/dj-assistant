# FFmpeg — sortir des builds GPL et non-free

**Ouvert** : 2026-08-02 · **Windows : livré** · **macOS : bloqué, aucune source retenue**

## Le problème, mesuré

Sift redistribue un binaire FFmpeg par plateforme, récupéré au bootstrap par
`scripts/fetch-ffmpeg.mjs`. Les trois sources épinglées ont été téléchargées et
inspectées le 2026-08-02 — pas déduites d'un nom de fichier, mais lues dans la ligne
`configuration:` du binaire lui-même (`ffmpeg -version` sur Windows ; recherche de la
chaîne dans le Mach-O pour les deux builds macOS, qui ne s'exécutent pas ici).

| Source | plateforme | `--enable-gpl` | `--enable-nonfree` |
| --- | --- | --- | --- |
| `ffmpeg-master-latest-win64-gpl.zip` (BtbN) | Windows | oui | non |
| `ffmpeg711arm.zip` (osxexperts) | macOS arm64 | **oui** | non |
| `ffmpeg7intel.zip` (osxexperts) | macOS x86_64 | **oui** | **oui** |

Les trois embarquent `libx264` et `libx265`, deux bibliothèques GPL-only.

## Pourquoi c'est du poids mort autant qu'un risque

Sift n'invoque que **trois encodeurs**, tous nommés en clair dans
`src-tauri/src/encode.rs:143-145` :

- `libmp3lame` — LAME, LGPL-2.1
- `pcm_s16be`, `pcm_s16le` — natifs FFmpeg

Côté décodage, FFmpeg n'a besoin que des décodeurs natifs (mp3, flac, alac, aac,
vorbis, opus, pcm). Aucun composant GPL-only n'est requis par le produit. Les drapeaux
GPL n'apportent donc rien à Sift : ils ne font qu'imposer leurs contraintes.

Deux conséquences de nature différente, à ne pas confondre :

- **`--enable-gpl`** rend le binaire GPL. Sift l'invoque en sidecar (processus séparé,
  `ffmpeg-sidecar`), ce qui est l'argument classique de l'agrégation — mais c'est une
  question juridique, pas technique, et elle n'est pas tranchée ici. Elle pèse sur le
  projet de vente (voir la mémoire `sift-cle-usb-etat-2026-08-02`, décision « licence
  de Sift »).
- **`--enable-nonfree`** ne demande aucune interprétation : la documentation de FFmpeg
  indique qu'un binaire construit avec ce drapeau **n'est redistribuable sous aucune
  licence**. Il est aujourd'hui dans l'installeur macOS Intel de Sift.

## Windows — livré

`scripts/fetch-ffmpeg.mjs` pointe désormais sur
`ffmpeg-master-latest-win64-lgpl.zip`. Vérifié sur le binaire réellement déposé dans
`src-tauri/binaries/` après un `npm run fetch-ffmpeg` :

- `--enable-gpl` absent, `--enable-nonfree` absent, `--enable-version3` présent
- `libmp3lame`, `pcm_s16be`, `pcm_s16le` présents
- décodeurs `mp3`, `flac`, `alac`, `aac`, `vorbis`, `opus`, `pcm_*` présents
- taille : 204 Mo (GPL) → 110 Mo (LGPL)

Le script cherche le binaire récursivement (`where /r`), donc le changement de nom du
dossier interne (`…-gpl/` → `…-lgpl/`) n'a demandé aucune autre modification.

## macOS — construit depuis les sources

**Aucun build LGPL macOS publiable n'existe.** Vérifié le 2026-08-02 :
`ColorsWind/FFmpeg-macOS` est bien LGPLv2, mais **partagé**
(`--enable-shared --disable-static`) et figé en **FFmpeg 5.0.1 depuis mai 2022** — des
dylibs à côté casseraient le sidecar mono-fichier, et quatre ans de retard sur les
décodeurs ne se rattrapent pas.

Le point qui a débloqué la question : **FFmpeg est LGPL par défaut.** Les builds publics
sont GPL parce qu'ils activent explicitement x264/x265, pas parce que le socle le serait.
Il n'y a donc pas besoin d'un `--disable-everything` chirurgical — il suffit de ne pas
passer `--enable-gpl` et d'ajouter `libmp3lame`. Ça écarte le vrai risque d'un build
minimal : oublier un démultiplexeur et casser un format d'entrée en silence, sur une
plateforme que **personne ici ne peut tester à la main** (pas de Mac disponible).

`scripts/build-ffmpeg-macos.sh` construit donc FFmpeg 8.1.2, épinglé par SHA256 calculée
sur le tarball réel (ffmpeg.org ne publie pas de `.sha256` — 404 — seulement une signature
GPG). Appelé par `fetch-ffmpeg.mjs`, donc CI et poste de dev suivent le même chemin.

Le script **échoue** plutôt que de livrer un binaire douteux. Cinq vérifications, chacune
pour un mode de défaillance distinct :

1. `--enable-gpl` / `--enable-nonfree` absents de la ligne `configuration:` — l'objectif.
2. `libmp3lame`, `pcm_s16be`, `pcm_s16le` présents — les trois encodeurs d'`encode.rs`.
3. `mp3`, `flac`, `alac`, `aac`, `vorbis`, `pcm_*` décodables — les formats d'entrée.
4. **`otool -L` ne montre que `/usr/lib` et `/System`.** Le piège de ce build : une
   dépendance vers `/opt/homebrew` passerait tous les tests sur le runner, où Homebrew
   existe, et casserait chez l'utilisateur, où il n'existe pas.
5. Un encodage MP3 320 réel, sur deux secondes de silence. Les listes disent qu'un
   encodeur est compilé, pas qu'il fonctionne.

Le build coûte plusieurs minutes ; `actions/cache` (clé = hash du script) évite de le
refaire tant que ni la version ni un drapeau de configure ne bougent.

**Ce qui reste ouvert** : aucune cible Intel n'est construite (`build.yml` n'a que
`aarch64-apple-darwin`), donc la source `ffmpeg7intel` non redistribuable n'était de toute
façon jamais atteinte par la CI. Si une cible Intel est ajoutée un jour, le même script la
couvre déjà.

## Trace

- `scripts/fetch-ffmpeg.mjs` — URL Windows sur le build LGPL ; macOS ne télécharge plus rien
  et appelle le script de compilation.
- `scripts/build-ffmpeg-macos.sh` — la compilation et ses cinq vérifications.
- `.github/workflows/{build,release}.yml` — cache de la compilation macOS, clé sur le hash du
  script.
- Vérifications de licence des anciennes sources faites dans le scratchpad de session, hors
  dépôt. Le résultat final est vérifié en CI, à chaque build.

### Ce que les six passages en CI ont appris

Le build macOS n'a pas marché du premier coup, et chaque échec a appris une chose distincte.
Trois d'entre elles n'étaient visibles que sur un vrai runner :

| # | Bloqué à | Nature |
| --- | --- | --- |
| 1 | décodeurs | faux positif — `set -o pipefail` + `grep -q` |
| 2 | dylibs X11/XCB | vrai défaut — `configure` liait ce qui traînait sur le runner |
| 3 | dylib LAME | vrai défaut — `ld` préférait la forme dynamique |
| 4 | `configure` | symptôme sans cause visible |
| 5 | — | vidage de `config.log`, aucun correctif |
| 6 | ✅ | `libmpg123` manquant, identifié par preuve |

Le passage 5 est celui qui a débloqué la série. Deux hypothèses prédisaient le même symptôme
(ordre des `-L`, chemin Homebrew implicite) ; en tenter une au hasard avait une chance sur deux
de « marcher » sans qu'on puisse dire pourquoi — un build qu'on ne sait plus expliquer. Le
vidage de `config.log` a coûté un tour et donné la vraie cause, qui n'était ni l'une ni l'autre.
Il reste dans le script.

La vérification `otool -L` a attrapé **deux** binaires (passages 2 et 3) qui passaient tous les
autres contrôles sur le runner et auraient refusé de démarrer chez l'utilisateur. C'est la seule
des cinq qui teste une machine absente, et c'est la plus rentable.
