// Mise en forme des deux mesures spectrales du Diagnostic, sans DOM.
//
// Module séparé de `report-view.ts` pour une raison de test, pas d'esthétique — même motif que
// `popover-position.ts` : `report-view.ts` importe `./ipc`, `@tauri-apps/api/core` et
// `wavesurfer.js` au niveau module, et la suite Vitest tourne en environnement Node sans Tauri
// (voir `vitest.config.ts`). Ici il n'y a que des nombres et des chaînes, donc la logique qui
// décide CE QUI EST DIT à l'utilisateur est couverte par un test — alors qu'elle porte
// précisément le risque : une mesure mal formulée devient un jugement.

/** Bornes des masters authentiques mesurés, en dB de platitude spectrale 16–20 kHz.
 *
 *  Ce ne sont PAS les seuils qui décident. Depuis le 2026-08-18 `verdict()` en applique un
 *  (`HF_FIXED_FLOOR_DB`, verdict.rs) — mais **pas celui-ci, et l'écart est délibéré** : ces
 *  bornes-là sont mesurées par `scripts/hf-flatness-probe.mjs` (mono 44,1 kHz forcé, DFT naïve,
 *  200 trames échantillonnées), le juge mesure au taux natif sur tout le fichier par la FFT de
 *  `spectrum.rs`. Sur les 10 authentiques du corpus, les deux chemins désaccordent jusqu'à 0,4 dB,
 *  assez pour faire basculer un achat. Tant que ces 44 fichiers n'ont pas été repassés par le
 *  chemin Rust, la borne d'affichage et la borne de décision restent deux nombres distincts.
 *
 *  Ce qui suit décrit donc l'ÉCHELLE affichée à côté de la valeur — une mesure sans échelle ne dit
 *  rien à personne — et pas la frontière du verdict.
 *
 *  **44 fichiers de provenance d'achat, trois familles musicales** (house/techno, ambient, broken
 *  beat acoustique), trois provenances indépendantes (Beatport magasin, Beatport clé USB, achats
 *  Bandcamp). Tous dans [-5,4 ; -2,5].
 *
 *  La borne basse n'est pas un point isolé : trois fichiers y convergent (-5,4 / -5,3 / -5,3).
 *  C'est ce qui la rend utilisable — un minimum unique serait un accident d'échantillon, un amas
 *  est une frontière. La distribution mesurée est d'ailleurs **bimodale** : 44 fichiers ici, puis
 *  un vide de 1,1 dB, puis 5 fichiers entre -12,7 et -6,5.
 *
 *  ⚠️ Ces 5-là sont EXCLUS de la référence, et c'est un jugement, pas une mesure. Sa raison : le
 *  vide qui les sépare, plus l'inspection spectrale de l'un d'eux, qui montre le contenu s'arrêter
 *  à ~18 kHz sur un plancher plat à -49 dB. Ce sont deux populations, pas une queue de
 *  distribution. Mais ils sont ACHETÉS eux aussi — donc ces bornes décrivent « un master à bande
 *  pleine », pas « un fichier légitime ». Un master volontairement sombre tombera dessous sans
 *  être fautif, et c'est pourquoi rien ici n'accuse.
 *
 *  Détail et méthode : `docs/superpowers/changes/2026-08-17-detecteur-corpus/review.md`.
 *
 *  Et surtout : **pas de jauge à trois crans.** Un fichier juste sous la borne n'est pas
 *  « suspect », il est juste sous la borne, et c'est tout ce qu'on peut en dire. */
export const HF_REF_LO = -5.4;
export const HF_REF_HI = -2.5;

/** Écart au-delà duquel la durée décodée s'affiche à côté de la durée déclarée, en secondes.
 *
 *  Un décodeur rend rarement EXACTEMENT ce que l'en-tête annonce — bourrage d'encodeur, arrondi de
 *  trame. Afficher les deux systématiquement noierait le seul cas qui informe sous du bruit
 *  permanent. Une seconde est très au-delà de ces écarts, tout en attrapant le cas réel : un
 *  fichier tronqué dont l'en-tête annonce encore la durée complète.
 *
 *  ⚠️ Choisie par raisonnement, **pas calibrée sur un corpus**, contrairement à `HF_REF_LO`. Le
 *  seul point mesuré est que les fichiers produits par ffmpeg rendent des durées identiques au
 *  centième. */
export const DURATION_MISMATCH_SEC = 1.0;

/** La densité de l'aigu telle qu'elle s'affiche : la mesure, puis sa référence.
 *
 *  Formulée comme un FAIT sur le fichier, jamais comme un verdict. « Ça a été du lossy » ne se
 *  déduit pas de cette mesure — un master volontairement sombre donne la même valeur. Le mot
 *  « densité » décrit ce qui est mesuré (à quel point la bande haute est remplie) là où
 *  « platitude » est du jargon et « clairsemé » est déjà un jugement. */
export function hfDensityParts(
  db: number,
  fmt: (v: number, d: number) => string,
): { value: string; ref: string } {
  const situe = db >= HF_REF_LO ? "dans" : "sous";
  return {
    value: `${fmt(db, 1)} dB`,
    ref: `${situe} la plage des masters (${fmt(HF_REF_LO, 1)} à ${fmt(HF_REF_HI, 1)})`,
  };
}

/** La durée décodée QUAND elle diverge de l'en-tête — `null` sinon, et la rangée ne se rend pas.
 *
 *  Le désaccord est l'information ; l'accord n'en est pas une. C'est aussi la seule mesure que
 *  Fakin' The Funk fait et que Sift ne faisait pas — sa classe CORROMPU en sort. La rangée
 *  « Durée » permanente est partie le 2026-09-07 (synthèse Détails techniques, décision
 *  d'Antoine : le lecteur affiche déjà la durée — un compte, un endroit) ; seul le cas divergent
 *  survit, rangé sous Intégrité où il parle.
 *
 *  `decodedSec <= 0` = pas mesuré (rapport antérieur à la mise en place, `#[serde(default)]` côté
 *  Rust) : jamais « 0 s réellement décodée ». */
export function decodedShortfallText(
  declaredSec: number,
  decodedSec: number,
  fmt: (v: number, d: number) => string,
): string | null {
  if (!(decodedSec > 0) || Math.abs(declaredSec - decodedSec) <= DURATION_MISMATCH_SEC) {
    return null;
  }
  return `${fmt(decodedSec, 1)} s sur ${fmt(declaredSec, 1)} s annoncées`;
}

/** Bornes des masters authentiques sur la bande RELATIVE au Nyquist, en dB.
 *
 *  **Beaucoup plus larges que celles de la bande fixe, et c'est la mesure qui l'impose.** Une
 *  première version annonçait -10,9, adossée aux 10 fichiers du corpus. Élargie à 32 authentiques
 *  de trois familles musicales, la borne tombe à **-23,8** — fixée par deux morceaux **ambient**
 *  (Nova Tekk, Chill Out Vol. 6), parfaitement normaux sur la bande fixe (-3,1 et -2,9) et très
 *  bas ici.
 *
 *  Autrement dit : **le haut du spectre d'un master ambient est légitimement clairsemé.** Avec
 *  l'ancienne borne, ces deux fichiers authentiques auraient été annoncés « sous la plage » — un
 *  faux positif sur du matériel acheté, pour un affichage censé ne rien accuser.
 *
 *  Le prix est réel : à ce seuil, cette bande ne détecte plus que 25 des 150 transcodages du
 *  corpus au lieu de 75. Elle reste utile — c'est la seule qui voit Opus (0/10 pour la bande fixe,
 *  6/10 ici) — mais elle est BEAUCOUP moins discriminante qu'annoncé d'abord.
 *
 *  Détail : `docs/superpowers/changes/2026-08-17-detecteur-corpus/review.md`. */
export const HF_TOP_REF_LO = -23.8;
export const HF_TOP_REF_HI = -2.5;

/** La densité du tout-haut du spectre, telle qu'elle s'affiche.
 *
 *  Mesure séparée et non remplaçante : les deux bandes sont aveugles à des endroits différents.
 *  Celle-ci est la seule qui voit Opus (0/10 pour la bande fixe, **6/10** ici) ; la fixe est la
 *  seule qui voit un MP3 128, dont la coupure passe en dessous d'elle.
 *
 *  ⚠️ Ce commentaire annonçait « 10/10 » pour Opus — le chiffre d'avant la correction de seuil du
 *  2026-08-18, resté ici alors que le bloc dix lignes plus haut le corrigeait déjà. Deux chiffres
 *  contradictoires dans le même fichier. */
export function hfTopDensityParts(
  db: number,
  fmt: (v: number, d: number) => string,
): { value: string; ref: string } {
  const situe = db >= HF_TOP_REF_LO ? "dans" : "sous";
  return {
    value: `${fmt(db, 1)} dB`,
    ref: `${situe} la plage des masters (${fmt(HF_TOP_REF_LO, 1)} à ${fmt(HF_TOP_REF_HI, 1)})`,
  };
}
