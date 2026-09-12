// Corpus ÉTALONNÉ pour le détecteur de faux lossless. Ouvert le 2026-08-17.
//
// Pourquoi il existe : le corpus de `make-fixtures.mjs` descend entièrement d'UN sinus balayé
// (`aevalsrc=0.3*sin(...)`) passé par UN encodeur à deux débits. Un sweep a de l'énergie pleine à
// chaque fréquence — c'est l'entrée la plus facile possible pour un détecteur de falaise, et vert
// dessus ne dit rien de la vraie musique. Ce script produit l'inverse : de vrais morceaux, une
// matrice d'encodeurs, et une VÉRITÉ TERRAIN par construction.
//
// Il prend N fichiers lossless dont la provenance est connue (achats Bandcamp/Beatport, rips CD),
// et pour chacun fabrique la matrice de transcodages ré-emballés en FLAC. Les originaux sont
// authentiques par provenance, les transcodages sont faux par construction — c'est ce qui en fait
// une vérité terrain, contrairement à l'avis d'un second logiciel qui ne serait qu'un second avis.
//
// N'ÉCRIT JAMAIS dans le dossier source. Toute la sortie est sous OUT.
//
//   node scripts/make-corpus.mjs <dossier-source> <dossier-sortie>
//   # puis, pour mesurer :
//   SIFT_CORPUS_DIR=<dossier-sortie>/genuine cargo test --manifest-path src-tauri/Cargo.toml \
//     --release corpus_scan -- --ignored --nocapture > genuine.csv
//   SIFT_CORPUS_DIR=<dossier-sortie>/fake    cargo test ... > fake.csv
//   node scripts/score-corpus.mjs <dossier-sortie>/labels.json <(cat genuine.csv fake.csv)
//
// ⚠️ Compter le coût avant de lancer : 10 sources de 6-7 min ont produit **17 Go** (les FLAC
// ré-emballés sont plus gros que l'original, voir la note sur `-sample_fmt` plus bas) et ~40 min
// d'encodage. La sortie est du dérivé : elle ne va jamais dans git, et elle se régénère.

import { execFileSync } from "node:child_process";
import { mkdirSync, readdirSync, existsSync, statSync, writeFileSync } from "node:fs";
import { join, extname, basename } from "node:path";

const [SRC, OUT] = process.argv.slice(2);
if (!SRC || !OUT) {
  console.error("usage: node scripts/make-corpus.mjs <dossier-source> <dossier-sortie>");
  process.exit(1);
}

const binDir = "src-tauri/binaries";
const found = readdirSync(binDir).find((f) => f.startsWith("ffmpeg-"));
if (!found) {
  console.error(`aucun ffmpeg dans ${binDir} — lancer d'abord: npm run fetch-ffmpeg`);
  process.exit(1);
}
const ff = join(binDir, found);

/// Un fichier de 0 octet N'EST PAS un fichier déjà fait.
///
/// Le premier passage de ce script a laissé un `.flac` vide derrière un encodage avorté, et le
/// `existsSync` nu du passage suivant l'a fait SAUTER : le corpus portait alors un artefact vide
/// étiqueté comme un vrai faux, et seul le compte d'erreurs du scan l'a rattrapé. C'est exactement
/// le défaut que le reste de ce chantier corrige — une absence qui a la forme d'une valeur.
const done = (p) => existsSync(p) && statSync(p).size > 0;

// Chaque entrée : [étiquette, args d'encodage, extension intermédiaire].
// Le choix couvre les trois axes qui manquent au corpus d'origine :
//   * plusieurs ENCODEURS pour le même format (libmp3lame vs mp3_mf, aac vs aac_mf) ;
//   * plusieurs FAMILLES de codec (MP3, AAC, Vorbis, Opus, WMA), qui ne coupent ni au même endroit
//     ni avec la même pente — et pour certaines, ne coupent pas du tout ;
//   * le HAUT de gamme, le cas dur : LAME 320 coupe vers 20,5 kHz, c'est-à-dire juste AU-DESSUS de
//     `verdict::LOSSLESS_OK_HZ` (20000). Les faux négatifs vivent là, pas à 128 kbps.
const MATRIX = [
  ["lame320", ["-c:a", "libmp3lame", "-b:a", "320k"], "mp3"],
  ["lame256", ["-c:a", "libmp3lame", "-b:a", "256k"], "mp3"],
  ["lame192", ["-c:a", "libmp3lame", "-b:a", "192k"], "mp3"],
  ["lame160", ["-c:a", "libmp3lame", "-b:a", "160k"], "mp3"],
  ["lame128", ["-c:a", "libmp3lame", "-b:a", "128k"], "mp3"],
  ["lameV0", ["-c:a", "libmp3lame", "-q:a", "0"], "mp3"],
  ["mfmp3_320", ["-c:a", "mp3_mf", "-b:a", "320k"], "mp3"],
  ["mfmp3_128", ["-c:a", "mp3_mf", "-b:a", "128k"], "mp3"],
  ["aac256", ["-c:a", "aac", "-b:a", "256k"], "m4a"],
  ["aac128", ["-c:a", "aac", "-b:a", "128k"], "m4a"],
  ["aacmf256", ["-c:a", "aac_mf", "-b:a", "256k"], "m4a"],
  ["aacmf128", ["-c:a", "aac_mf", "-b:a", "128k"], "m4a"],
  ["vorbisq5", ["-c:a", "libvorbis", "-q:a", "5"], "ogg"],
  ["opus128", ["-c:a", "libopus", "-b:a", "128k"], "opus"],
  ["wma192", ["-c:a", "wmav2", "-b:a", "192k"], "wma"],
];

const AUDIO = new Set([".aif", ".aiff", ".wav", ".flac"]);
const sources = readdirSync(SRC).filter((f) => AUDIO.has(extname(f).toLowerCase()));
if (sources.length === 0) {
  console.error(`aucun fichier lossless dans ${SRC}`);
  process.exit(1);
}

const genuineDir = join(OUT, "genuine");
const fakeDir = join(OUT, "fake");
const tmpDir = join(OUT, "_tmp");
for (const d of [genuineDir, fakeDir, tmpDir]) mkdirSync(d, { recursive: true });

// Deux options que le premier passage n'avait pas, et qui l'ont fait échouer ou biaiser :
//
// `-vn` — les AIFF achetés portent une pochette embarquée, que ffmpeg présente comme un flux
//   vidéo. Le muxer `.m4a` (ipod) la refuse — « Could not find tag for codec h264 in stream #0 » —
//   et les QUATRE variantes AAC échouaient toutes pour cette raison, pas parce que l'encodeur
//   manquait. Inséré juste avant le fichier de sortie et pas en tête : placé avant `-i`, `-vn` est
//   une option d'ENTRÉE, ce qui n'est pas la même chose.
//
// `-sample_fmt s16` sur le ré-emballage — un décodeur lossy rend du flottant, et l'encodeur FLAC
//   promeut alors en s32 (24 bit). Sans cette option, TOUS les faux étaient en 24 bit et tous les
//   authentiques en 16 : le corpus devenait séparable par la profondeur de bits seule, un artefact
//   du pipeline et pas de la fraude — et un vrai faux lossless du monde réel est presque toujours
//   en 16 bit, puisqu'il se fait passer pour un rip CD. Mesuré le 2026-08-17 sur quatre variantes :
//   le cutoff est identique en s16 et en s32 (22050 / 16860 / 22050, et 20215 contre 20227 pour
//   Opus, soit sous la résolution d'un bin), donc ce biais NE PORTAIT PAS la mesure de l'époque —
//   mais il est retiré pour que le corpus n'ait qu'une seule variable.
const run = (args) => {
  const out = args[args.length - 1];
  execFileSync(ff, ["-hide_banner", "-loglevel", "error", ...args.slice(0, -1), "-vn", out]);
};

const label = [];
let made = 0;
let failed = 0;

for (const [i, f] of sources.entries()) {
  const stem = `src${String(i + 1).padStart(2, "0")}`;
  const srcPath = join(SRC, f);

  // Côté AUTHENTIQUE : ré-encodage FLAC depuis la source lossless. FLAC est un conteneur SANS
  // PERTE, donc le spectre est identique à l'original — ce n'est pas un transcodage. On uniformise
  // le conteneur pour que la seule variable du corpus soit l'encodage subi, jamais le format
  // déclaré.
  //
  // Dans un try/catch comme le reste : sans lui, UNE source illisible tuait tout le run et le
  // corpus s'arrêtait en silence à la source où ça s'était produit (constaté au premier passage).
  const gen = join(genuineDir, `${stem}_genuine.flac`);
  try {
    if (!done(gen)) run(["-y", "-i", srcPath, "-c:a", "flac", "-sample_fmt", "s16", gen]);
    label.push({ file: basename(gen), truth: "genuine", source: f, via: "-" });
  } catch (e) {
    failed++;
    console.error(`ECHEC ${stem} genuine: ${String(e).split("\n")[0]}`);
    continue; // pas de source authentique = pas de faux à en tirer
  }

  for (const [name, args, ext] of MATRIX) {
    const mid = join(tmpDir, `${stem}_${name}.${ext}`);
    const out = join(fakeDir, `${stem}_${name}.flac`);
    if (done(out)) {
      label.push({ file: basename(out), truth: "fake", source: f, via: name });
      made++;
      continue;
    }
    try {
      run(["-y", "-i", srcPath, ...args, mid]);
      // Ré-emballage en FLAC : le fichier DÉCLARE lossless alors que son contenu a subi une perte.
      // C'est exactement la fraude que le détecteur doit voir.
      run(["-y", "-i", mid, "-c:a", "flac", "-sample_fmt", "s16", out]);
      label.push({ file: basename(out), truth: "fake", source: f, via: name });
      made++;
    } catch (e) {
      // Un encodeur absent ou en échec est une LIGNE du résultat, pas un silence : sinon un corpus
      // incomplet passe pour un corpus propre.
      failed++;
      console.error(`ECHEC ${stem} ${name}: ${String(e).split("\n")[0]}`);
    }
  }
  console.error(`[${i + 1}/${sources.length}] ${f}`);
}

writeFileSync(join(OUT, "labels.json"), JSON.stringify(label, null, 2));
console.error(
  `\n${sources.length} sources, ${made} faux fabriqués, ${failed} échecs, ` +
    `${label.length} lignes étiquetées -> ${join(OUT, "labels.json")}`,
);
