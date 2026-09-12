#!/usr/bin/env bash
# Gate de vérification déterministe consommée par le hook Stop déclaré dans
# .claude/settings.json (côté PROJET, versionné). Sortie non-zéro = le tour est
# bloqué et cette sortie est réinjectée. C'est la mise en œuvre de la règle C1 :
# « terminé = démontrable », côté machine plutôt que côté discipline.
#
# ⚠️ Cet en-tête a nommé `~/.claude/stop-verify.sh` jusqu'au 2026-08-18. Ce
# fichier n'existe plus — supprimé par le reset vanilla de ~/.claude du
# 2026-07-31, et le déclenchement est passé côté projet par `f9fa086` le
# 2026-08-11. Un pointeur vers un fichier absent ne tombe pas, il se lit.
#
# CONTRAINTE : ce script tourne à CHAQUE fin de tour. Il ne contient donc que
# des vérifications rapides. Budget mesuré le 2026-07-28 :
#   npx tsc --noEmit    3,4 s
#   npm run lint:tokens 0,7 s
#   cargo check         2,1 s en incrémental (49 s à froid, une seule fois)
# puis le 2026-08-26 :
#   cargo fmt --check   0,58 s
#
# La suite de tests complète et `cargo test` restent hors gate : ils appartiennent
# au pre-commit et à la CI, pas à la fin de tour.
#
# ⚠️ AUCUN COMPTE DE TESTS ICI, et c'est délibéré. Cette ligne a annoncé « 399 cas »
# et le commentaire de la borne cargo « 417 tests » ; les deux étaient faux au
# 2026-08-18 (mesure : 51 tests Vitest sur 8 fichiers, 583 attributs #[test] Rust
# dont 573 joués et 19 ignorés). Un nombre figé dans un commentaire ne devient pas
# faux bruyamment, il devient faux en silence — même leçon que le témoin de
# lint-tokens.mjs (issue #29) et que le chiffrage calculé de tests.yml côté Tuple.
# Le compte du jour se lit : `npm run test` et
# `cargo test --manifest-path src-tauri/Cargo.toml`.
set -u

cd "$(dirname "$0")/.." || exit 0
rc=0

run() {
  local nom="$1"; shift
  local out
  if ! out=$("$@" 2>&1); then
    printf '### %s : ECHEC\n%s\n\n' "$nom" "$(printf '%s' "$out" | tail -40)"
    rc=1
  fi
}

run "typecheck (tsc --noEmit)" npx tsc --noEmit
run "tokens (lint:tokens)" npm run -s lint:tokens
# 0,164 s mesurées — il lit des fichiers et ne compile rien. Il garde le seul défaut que la
# relecture humaine ne voit pas : un commentaire français désaccentué se lit sans qu'on le
# remarque (issues #43/#44). Ratchet à baseline, comme lint:tokens.
run "accents (lint:accents)" npm run -s lint:accents
# `*/` orphelin dans styles.css : payé DEUX FOIS les 2026-09-06/07 — le navigateur lit le texte
# hors /* */ comme un sélecteur invalide et ignore EN SILENCE la règle suivante ; aucune autre
# gate ne le voit (le CSS reste syntaxiquement parsable). Attrapé les deux fois par une mesure
# CDP qui contredisait la feuille — d'où cette gate, mesurée en mutation à sa création.
run "css-comments (lint:css-comments)" npm run -s lint:css-comments

# `cargo fmt --check` : ajouté le 2026-08-26 après un raté qu'aucune gate locale
# ne pouvait voir. `34e7f12` a livré trois emplacements de `ipc_filing.rs` non
# formatés ; la CI (`test.yml`, étape Rustfmt) est restée ROUGE sur `main`
# pendant quatre commits sans que rien ne le signale ici, parce que cette gate
# de fin de tour s'arrêtait à `tsc` + `lint:tokens` + `cargo check`.
#
# Il tient dans le budget d'en-tête là où `clippy` et `cargo test` n'y tiennent
# pas : rustfmt ne compile RIEN — il parse et reformate. 0,58 s mesurées, et
# surtout il ne prend pas le lock du `target/`, donc il n'a besoin d'aucune des
# bornes qui encadrent `cargo check` ci-dessous et reste vrai pendant qu'un
# `tauri dev` compile.
if [ -f src-tauri/Cargo.toml ]; then
  run "format (cargo fmt --check)" cargo fmt --manifest-path src-tauri/Cargo.toml --check
fi

# Rust : uniquement si le crate existe. `cargo check` et non `cargo build` —
# on veut l'erreur de type, pas l'artefact.
#
# BORNE DE TEMPS obligatoire : cargo prend le lock du target dir. Si un
# `tauri dev` compile en parallele, cargo ATTEND le lock au lieu d'echouer, et
# la fin de tour serait bloquee le temps de son rebuild. Le passage a froid
# (49 s mesurees) tombe sous la meme borne. Au-dela de 25 s on abandonne la
# verification Rust pour ce tour SANS echouer : une gate de fin de tour doit
# etre rapide ou muette, jamais lente. Le filet reste le hook pre-commit, qui
# lui execute la suite complete avec un budget de 300 s. (Le compte de tests qui
# figurait ici est retire pour la raison donnee en tete de fichier.)
if [ -f src-tauri/Cargo.toml ]; then
  log=$(mktemp "${TMPDIR:-/tmp}/verify-cargo-XXXXXX")
  if command -v timeout >/dev/null 2>&1; then
    ( cd src-tauri && timeout 25 cargo check --quiet ) >"$log" 2>&1
  else
    ( cd src-tauri && cargo check --quiet ) >"$log" 2>&1
  fi
  crc=$?
  if [ "$crc" -eq 124 ]; then
    printf '### rust (cargo check) : IGNORE ce tour (>25 s, lock cargo ou build a froid)\n\n'
  elif [ "$crc" -ne 0 ]; then
    printf '### rust (cargo check) : ECHEC\n%s\n\n' "$(tail -40 "$log")"
    rc=1
  fi
  rm -f "$log"
fi

exit $rc
