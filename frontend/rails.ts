// Rail lossless / lossy déduit de la SEULE extension — la copie frontend unique de la table Rust.
//
// Module feuille, ZÉRO import (même précédent que `popover-position.ts`) : c'est ce qui lui permet
// d'être lu par `filing.ts`, chaîné à `./ipc` et donc jamais chargeable en environnement Node,
// aussi bien que par `library-views.ts`, sans rapprocher deux modules qui n'ont rien à se dire.
//
// Recopie volontaire, et bornée, de `analysis::tags::rail_from_ext` (src-tauri/src/analysis/tags.rs
// lignes 24-30). Elle a divergé DEUX fois, toujours de la même façon — une table écrite de mémoire
// à côté d'une autre :
//   · la version qui vivait dans `filing.ts` ignorait `opus` ;
//   · `library-views.ts` portait sa PROPRE table (`LOSSLESS_EXT`) qui ignorait `aif`, si bien qu'un
//     `.aif` authentique lisait AUTHENTIQUE au lieu de LOSSLESS dans la colonne Verdict (constaté et
//     corrigé le 2026-08-20 en ramenant les deux ici).
// Toute correction ici doit d'abord être vérifiée là-bas — c'est le backend qui fait autorité.

/** Mêmes trois valeurs que `declared_rail` / `QueueItem.rail` dans `shared/contracts.ts`. */
export type Rail = "lossless" | "lossy" | "unknown";

const LOSSLESS_EXT = new Set(["flac", "wav", "aif", "aiff", "alac"]);
const LOSSY_EXT = new Set(["mp3", "aac", "m4a", "ogg", "opus"]);

/** Rail d'une EXTENSION nue (« aiff », « MP3 ») — la forme que porte `LibraryTrack.format`, écrite
 *  par Sift au rangement (`library.rs`, `target_format` → `Target::ext()`), donc sans point. */
export function railFromExt(ext: string): Rail {
  const e = ext.toLowerCase();
  if (LOSSLESS_EXT.has(e)) return "lossless";
  if (LOSSY_EXT.has(e)) return "lossy";
  return "unknown";
}

/** Rail d'un CHEMIN — enveloppe `railFromExt` sur ce qui suit le dernier point. Un chemin sans
 *  point rend `unknown` par le même chemin qu'une extension inconnue. */
export function railFromExtension(path: string): Rail {
  return railFromExt(path.split(".").pop() || "");
}
