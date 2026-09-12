// Sonde de PLATITUDE SPECTRALE DE L'AIGU — candidat pour remplacer/compléter la coupure.
//
// Pourquoi : `detect_cutoff` (spectrum.rs) et Fakin' The Funk reposent tous deux sur la POSITION
// d'une falaise spectrale. Mesuré le 2026-08-17 sur 150 transcodages étiquetés, ce signal plafonne
// à ~27 % de détection, et il est structurellement aveugle à l'AAC, au LAME 320, au V0, à Opus,
// Vorbis et WMA — leur coupure est à 22050, il n'y a rien à seuiller.
//
// L'idée : un encodeur lossy ne supprime pas l'aigu, il ne garde que ses coefficients les plus
// forts et met le reste à zéro. L'aigu devient CLAIRSEMÉ ET POINTU, là où un master porte un
// plancher de bruit continu (dither + contenu naturel). Ça se mesure par la platitude spectrale
// — moyenne géométrique sur moyenne arithmétique — de la bande 16-20 kHz, médiane sur les trames.
//
// Ce n'est PAS la position d'une coupure : ça marche même quand il n'y en a aucune.
//
//   node scripts/hf-flatness-probe.mjs <fichier.flac> [...]
//
// Résultat mesuré le 2026-08-18 sur le corpus étiqueté (voir review.md) :
//   20 authentiques (2 provenances) : -5,4 à -2,6 dB
//   150 transcodages               : jusqu'à -43,8 dB, 91 sous le seuil
//   détection 61 % contre 27 % pour la coupure, 0 faux positif
//   angle mort restant : Opus (0/10) — il ne creuse pas l'aigu.
import { execFileSync } from "node:child_process";
import { readdirSync } from "node:fs";
import { join, basename } from "node:path";

const SR = 44100, N = 4096, HOP = 2048;
const binDir = "src-tauri/binaries";
const ff = join(binDir, readdirSync(binDir).find((f) => f.startsWith("ffmpeg-")));

/** Décodage mono 44,1 kHz en f32, borné à `maxSec` — la médiane sur trames n'a pas besoin du
 *  morceau entier, et ça garde la sonde utilisable sur une bibliothèque. */
function decode(path, maxSec = 150) {
  const buf = execFileSync(ff, ["-v","quiet","-i",path,"-map","0:a","-t",String(maxSec),
    "-f","f32le","-ac","1","-ar",String(SR),"-"], { maxBuffer: 1 << 30 });
  return new Float32Array(buf.buffer, buf.byteOffset, Math.floor(buf.length / 4));
}

const hann = Float64Array.from({ length: N }, (_, i) => 0.5 - 0.5 * Math.cos((2 * Math.PI * i) / (N - 1)));

/** DFT réelle naïve limitée aux bins qui nous intéressent — on ne veut que 16-20 kHz, soit ~372
 *  bins sur 2048, donc une FFT complète serait du gâchis dans une sonde. */
function bandPower(frame, loBin, hiBin) {
  const out = new Float64Array(hiBin - loBin);
  for (let k = loBin; k < hiBin; k++) {
    let re = 0, im = 0;
    const w = (-2 * Math.PI * k) / N;
    for (let n = 0; n < N; n++) { const a = w * n; re += frame[n] * Math.cos(a); im += frame[n] * Math.sin(a); }
    out[k - loBin] = re * re + im * im;
  }
  return out;
}

function flatnessDb(x) {
  const hzPerBin = SR / N;
  const lo = Math.ceil(16000 / hzPerBin), hi = Math.floor(20000 / hzPerBin);
  const nfr = 1 + Math.floor((x.length - N) / HOP);
  if (nfr < 100) return null;
  const vals = [];
  // Un échantillonnage régulier des trames suffit à une médiane et évite un coût quadratique.
  const step = Math.max(1, Math.floor(nfr / 200));
  const frame = new Float64Array(N);
  for (let f = 0; f < nfr; f += step) {
    const off = f * HOP;
    for (let n = 0; n < N; n++) frame[n] = x[off + n] * hann[n];
    const P = bandPower(frame, lo, hi);
    let logSum = 0, sum = 0;
    for (const p of P) { const q = p + 1e-20; logSum += Math.log(q); sum += q; }
    vals.push(10 * Math.log10(Math.exp(logSum / P.length) / (sum / P.length)));
  }
  vals.sort((a, b) => a - b);
  return vals[Math.floor(vals.length / 2)];
}

for (const p of process.argv.slice(2)) {
  const v = flatnessDb(decode(p));
  console.log(`${basename(p).padEnd(30)} ${v === null ? "(trop court)" : v.toFixed(1) + " dB"}`);
}
