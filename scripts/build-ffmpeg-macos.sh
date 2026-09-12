#!/usr/bin/env bash
# Construit un ffmpeg macOS SANS composant GPL, pour remplacer les builds osxexperts épinglés
# jusqu'au 2026-08-02 : `ffmpeg711arm` était --enable-gpl, et `ffmpeg7intel` --enable-gpl ET
# --enable-nonfree (un binaire nonfree n'est redistribuable sous aucune licence).
#
# POURQUOI PAS `--disable-everything` : FFmpeg est LGPL PAR DÉFAUT. Les builds publiés sont GPL
# parce qu'ils activent explicitement x264/x265, pas parce que le socle le serait. Il suffit donc
# de ne PAS passer --enable-gpl et d'ajouter libmp3lame (LAME est LGPL-2.1). On garde ainsi tous
# les codecs natifs, et on évite le vrai risque d'un build chirurgical : oublier un démultiplexeur
# et casser un format d'entrée en silence, sur une plateforme que personne ici ne peut tester.
#
# Sift n'utilise que trois encodeurs (`src-tauri/src/encode.rs:143-145`) mais décode des fichiers
# utilisateurs arbitraires : la générosité du côté décodage est voulue.
#
# Appelé par `scripts/fetch-ffmpeg.mjs` sur macOS. Usage :
#   scripts/build-ffmpeg-macos.sh <chemin/du/binaire/de/sortie>

set -euo pipefail

OUT="${1:?usage: build-ffmpeg-macos.sh <output-binary-path>}"

# Version épinglée. La mettre à jour est un geste délibéré, pas une dérive : `latest` ferait
# changer le binaire distribué sans qu'aucun commit ne le dise. 8.1.2 est la dernière release
# stable au 2026-08-02 ; elle rapproche macOS du build Windows, qui tourne sur un instantané de
# master (N-125881), là où l'ancienne source osxexperts restait en 7.1.1.
FFMPEG_VERSION="8.1.2"
# Empreinte CALCULÉE sur le tarball réellement téléchargé depuis ffmpeg.org le 2026-08-02, pas
# recopiée d'une page. ffmpeg.org ne publie pas de .sha256 (404) — seulement une signature GPG,
# dont l'usage demanderait d'épingler en plus une empreinte de clé. L'épingle ci-dessous protège
# donc contre une corruption de transfert et contre un changement silencieux du contenu sous une
# même version ; elle ne remplace pas une vérification de signature, et ne prétend pas le faire.
# La mettre à jour en même temps que FFMPEG_VERSION, jamais séparément.
FFMPEG_SHA256="464beb5e7bf0c311e68b45ae2f04e9cc2af88851abb4082231742a74d97b524c"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

echo "==> Dependances (LAME + mpg123)"
# LAME fournit libmp3lame, le seul encodeur externe dont Sift a besoin.
#
# mpg123 n'est PAS un caprice : le LAME 4.0 de Homebrew délègue son décodeur à libmpg123
# (`hip123_decode1` dans mpglib_interface.o). La dylib porte cette dépendance en elle, l'archive
# statique attend qu'on la lie soi-même. Sans elle, le link échoue sur une trentaine de symboles
# `_mpg123_*` — diagnostic obtenu en CI le 2026-08-02 via le vidage de config.log plus bas.
# Sift n'utilise que l'ENCODEUR (ffmpeg décode le MP3 avec son décodeur natif), mais l'archive
# est monolithique : on ne peut pas prendre l'encodeur sans son voisin. mpg123 est LGPL-2.1,
# donc sans effet sur la licence du résultat.
brew list lame >/dev/null 2>&1 || brew install lame
brew list mpg123 >/dev/null 2>&1 || brew install mpg123
brew list nasm >/dev/null 2>&1 || brew install nasm
LAME_PREFIX="$(brew --prefix lame)"
MPG123_PREFIX="$(brew --prefix mpg123)"

# Le dossier Homebrew contient l'archive statique ET la dylib, et `ld` préfère la dylib quand les
# deux sont visibles : pointer `-L` vers ce dossier laissait le choix au linker, qui prenait le
# mauvais (attrapé en CI par la vérification otool). On copie les seules archives dans un dossier
# à nous — le linker n'a plus d'alternative.
#
# Vérifier la PRÉSENCE d'une archive ne prouve pas qu'elle sera UTILISÉE : c'est précisément
# l'erreur qui a produit un binaire dépendant de /opt/homebrew tout en passant ce contrôle.
LAME_STATIC="$WORK/static-libs"
mkdir -p "$LAME_STATIC"
for lib in "$LAME_PREFIX/lib/libmp3lame.a" "$MPG123_PREFIX/lib/libmpg123.a"; do
  if [ ! -f "$lib" ]; then
    echo "ERREUR: $lib absent — sans archive statique, ffmpeg lierait la dylib Homebrew," >&2
    echo "        introuvable sur la machine de l'utilisateur." >&2
    exit 1
  fi
  cp "$lib" "$LAME_STATIC/"
done

echo "==> Sources FFmpeg $FFMPEG_VERSION"
cd "$WORK"
curl -fsSL -o ffmpeg.tar.xz "https://ffmpeg.org/releases/ffmpeg-$FFMPEG_VERSION.tar.xz"
echo "$FFMPEG_SHA256  ffmpeg.tar.xz" | shasum -a 256 -c - || {
  echo "ERREUR: l'archive telechargee ne correspond pas a FFMPEG_SHA256." >&2
  echo "        Attendu : $FFMPEG_SHA256" >&2
  echo "        Obtenu  : $(shasum -a 256 ffmpeg.tar.xz | awk '{print $1}')" >&2
  exit 1
}
tar xf ffmpeg.tar.xz
cd "ffmpeg-$FFMPEG_VERSION"

echo "==> configure (LGPL : ni --enable-gpl ni --enable-nonfree)"
# --disable-autodetect : LE drapeau qui rend ce build reproductible. Sans lui, `configure` lie
#   tout ce qu'il trouve installé sur la machine. Sur le runner GitHub, Homebrew fournit libxcb,
#   et ffmpeg activait donc `xcbgrab` (capture d'écran X11, dont Sift n'a aucun usage) en tirant
#   six dylibs de /opt/homebrew -- introuvables sur le Mac de l'utilisateur. Attrapé en CI le
#   2026-08-02 par la vérification otool. Désactiver ces six-là une par une aurait été du
#   colmatage : le prochain runner avec une autre bibliothèque installée aurait rejoué la scène.
#   BtbN fait le même choix sur son build LGPL Windows (--disable-libxcb --disable-xlib).
#   Conséquence : tout ce dont on a besoin doit être demandé EXPLICITEMENT ci-dessous.
# --enable-zlib : réclamé explicitement puisque l'autodétection est coupée. Présente en système
#   (/usr/lib/libz), donc conforme à l'invariant otool.
# --disable-network : Sift ne lit que des fichiers locaux. Rien à gagner à garder les protocoles
#   réseau, et c'est autant de surface d'attaque en moins sur un binaire qui traite des fichiers
#   téléchargés par l'utilisateur.
# --disable-ffplay/--disable-ffprobe : seul `ffmpeg` est bundlé comme sidecar.
./configure \
  --prefix="$WORK/prefix" \
  --enable-static \
  --disable-shared \
  --pkg-config-flags=--static \
  --disable-autodetect \
  --extra-cflags="-I$LAME_PREFIX/include" \
  --extra-ldflags="-L$LAME_STATIC" \
  --extra-libs="-lmpg123" \
  --enable-libmp3lame \
  --enable-zlib \
  --disable-network \
  --disable-ffplay \
  --disable-ffprobe \
  --disable-doc \
  --disable-debug || {
  # `configure` n'imprime qu'un « X not found » et écrit la VRAIE erreur du compilateur ou du
  # linker dans config.log, qu'aucune CI ne lit. Sans ce vidage, chaque échec ici coûte un
  # aller-retour complet pour deviner. On le sort donc à l'échec, jamais autrement.
  echo "===== ffbuild/config.log (60 dernières lignes) =====" >&2
  tail -n 60 ffbuild/config.log >&2 2>/dev/null || tail -n 60 config.log >&2 2>/dev/null || true
  exit 1
}

echo "==> make"
make -j"$(sysctl -n hw.ncpu)"

BUILT="$WORK/ffmpeg-$FFMPEG_VERSION/ffmpeg"

# Les listes sont capturées UNE fois dans des variables, et les recherches se font dessus par
# here-string. Ne jamais écrire `"$BUILT" -hide_banner -decoders | grep -q ...` : `grep -q` sort
# dès la première correspondance, ce qui envoie un SIGPIPE à ffmpeg encore en train d'écrire, et
# `set -o pipefail` remonte alors 141 comme statut du pipeline ALORS QUE la correspondance a été
# trouvée. Le test échouait donc selon la position alphabétique de ce qu'il cherchait : `aac`,
# premier de la liste des décodeurs, faisait couper tôt ; `libmp3lame`, en fin de liste des
# encodeurs, laissait ffmpeg terminer. Observé en CI le 2026-08-02.
CONFIG="$("$BUILT" -hide_banner -version)"
ENCODERS="$("$BUILT" -hide_banner -encoders)"
DECODERS="$("$BUILT" -hide_banner -decoders)"

echo "==> Verification : licence"
for flag in --enable-gpl --enable-nonfree; do
  if grep -q -- "$flag" <<<"$CONFIG"; then
    echo "ERREUR: le binaire construit porte $flag." >&2
    exit 1
  fi
done

echo "==> Verification : les encodeurs dont Sift a besoin"
# `encode.rs:143-145`. Un binaire qui compile mais ne sait pas encoder en MP3 casserait la
# fonction centrale de l'app, sur une plateforme que personne ici ne peut tester à la main.
for enc in libmp3lame pcm_s16be pcm_s16le; do
  grep -qE "^ [A-Z.]+ $enc " <<<"$ENCODERS" || {
    echo "ERREUR: encodeur $enc absent du binaire construit." >&2
    exit 1
  }
done

echo "==> Verification : les decodeurs des formats d'entree"
for dec in mp3 flac alac aac vorbis pcm_s16le pcm_s16be; do
  grep -qE "^ [A-Z.]+ $dec " <<<"$DECODERS" || {
    echo "ERREUR: decodeur $dec absent du binaire construit." >&2
    exit 1
  }
done

echo "==> Verification : aucune dylib non systeme"
# LE piège de ce build. Une dépendance vers /opt/homebrew ou /usr/local passerait tous les tests
# ci-dessus sur le runner — où Homebrew existe — et casserait chez l'utilisateur, où il n'existe
# pas. Seules /usr/lib et /System sont présentes sur toute machine macOS.
BAD="$(otool -L "$BUILT" | tail -n +2 | awk '{print $1}' | grep -vE '^(/usr/lib/|/System/)' || true)"
if [ -n "$BAD" ]; then
  echo "ERREUR: dependances dynamiques non systeme, absentes sur la machine de l'utilisateur :" >&2
  printf '  %s\n' $BAD >&2
  exit 1
fi

echo "==> Verification : un encodage MP3 reel"
# Les listes ci-dessus disent qu'un encodeur est compilé, pas qu'il fonctionne. Deux secondes de
# silence encodées en MP3 320, c'est exactement le chemin de production.
"$BUILT" -hide_banner -loglevel error -f lavfi -i "anullsrc=r=44100:cl=stereo" -t 2 \
  -vn -c:a libmp3lame -b:a 320k -ar 44100 -y "$WORK/probe.mp3"
[ -s "$WORK/probe.mp3" ] || { echo "ERREUR: l'encodage MP3 de controle n'a produit aucun octet." >&2; exit 1; }

mkdir -p "$(dirname "$OUT")"
cp "$BUILT" "$OUT"
chmod 755 "$OUT"
echo "==> OK : $OUT ($(wc -c < "$OUT") octets)"
