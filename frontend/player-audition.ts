// Rangée d'audition du lecteur de Revue (play · slider de progression · temps · volume fin) —
// module PUR (aucun import `./ipc`, aucun DOM) extrait de `report-view.ts` le 2026-08-27, pour que
// la story exécute le VRAI markup au lieu d'en recopier un — même motif que `rail-source-entry.ts`
// et `queue-verdict-dot.ts`. Le markup est STATIQUE (aucune donnée utilisateur, pas d'`esc()` à
// avoir) : tout l'état — remplissages, pouce, icône de mute, aria-valuenow — est muté après coup
// par `mountPlayer` (`report-view.ts`), qui reste l'unique appelant de prod via `playerRowHtml`.
// Les deux formules partagées ci-dessous sont exportées pour la même raison : le rendu de la story
// et celui de la prod ne peuvent pas diverger s'ils appellent le même code.

/** Diamètre du pouce du volume (maquette « Volume (lecteur) », façon Music). Le pouce de la
 *  progression fait 20 (kit Pickers/Linear/Small) — lui n'entre dans aucune formule JS, le pouce
 *  suit `left:pct%` plein-largeur. */
export const VOL_KNOB = 14;

/** Course du CENTRE du pouce du volume, en calc CSS — `left` du pouce ET `width` du remplissage
 *  (le pouce mène, le remplissage s'arrête à son centre, kit). Même formule que la conversion
 *  pointeur → valeur de `dragSlider` (report-view.ts), leçon du 2026-08-25 : mapper la largeur
 *  entière faisait traîner le pouce derrière le pointeur. */
export function volumeCentreCss(pct: number): string {
  return `calc(${pct} * (100% - ${VOL_KNOB}px) + ${VOL_KNOB / 2}px)`;
}

/** L'icône du haut-parleur dit l'état muet (bascule webfont) — la capsule montrait un slash
 *  permanent, le slider fin suit Music : volume coupé = glyphe barré. */
export function volumeIconClass(pct: number): string {
  return pct > 0 ? "ti ti-volume" : "ti ti-volume-off";
}

/** Le markup de la rangée, tel que `playerRowHtml` l'insère dans `.sift-player-row`. */
export function playerAuditionHtml(): string {
  return (
    `<div class="sift-player-audition">` +
    // Glyphe PLEIN (2026-08-27, motif Music) : classe .ti-fill du dépôt, police
    // tabler-icons-filled — setIcon (report-view.ts) bascule play/pause dans la même famille.
    `<button class="sift-play sift-play-btn" title="Lecture / pause (espace)" aria-label="Lecture / pause (espace)"><i class="ti-fill ti-fill-player-play"></i></button>` +
    // LECTEUR SIMPLE (décision Antoine 2026-08-27, maquette : composant « Slider de progression »,
    // COPIE du kit Pickers/Slider-pickers/Linear/Small/No-tick-marks 53:118) : la waveform quitte
    // Revue — piste 4 px + remplissage accent + pouce blanc 20, la géométrie exacte du kit.
    // WaveSurfer RESTE le moteur audio (décodage, lecture, seek, volume) : son conteneur
    // `.sift-wave` est réduit à zéro par CSS (.sift-progress-engine), jamais démonté — le passer
    // en display:none casserait son ResizeObserver, le réduire ne casse rien.
    // Survol : bulle mm:ss seule (patron QuickTime) — le ghost et la ligne, nés pour teinter des
    // BARRES, n'ont pas d'équivalent sur une piste pleine de 4 px.
    `<div class="sift-progress" role="slider" tabindex="0" aria-label="Position de lecture" aria-valuemin="0" aria-valuemax="100" aria-valuenow="0">` +
    `<div class="sift-wave sift-player-wave sift-progress-engine"></div>` +
    `<div class="sift-progress-track"></div>` +
    `<div class="sift-progress-fill"></div>` +
    `<div class="sift-progress-knob" hidden></div>` +
    `<div class="sift-wave-hovertime" hidden></div>` +
    `</div>` +
    // Temps À CÔTÉ de l'onde (retour Antoine : plus overlay dans la forme d'onde). Un seul, cliquable.
    `<span class="sift-time" role="button" tabindex="0" title="Temps écoulé / restant — cliquer pour basculer">0:00</span>` +
    // Volume intégré dans la rangée de transport (façon Apple Music) — plus de bloc « contrôles »
    // séparé. Tempo & key-lock (l'« Écoute avancée ») retirés : le pitch DJ n'est pas voulu sur cet
    // écran de décision (Antoine 2026-08-21), et la HIG ne justifie un contrôle audio custom que pour
    // une commande absente du système.
    // SLIDER FIN (2026-08-27, remplace la capsule SVG du 25 — « goofy » dans la rangée fine) :
    // même famille que .sift-progress (patron Music, maquette « Volume (lecteur) ») — haut-parleur
    // cliquable (mute, bascule ti-volume/ti-volume-off) + piste 4 px, remplissage et pouce BLANCS
    // theme-invariants (un volume n'est pas une progression : pas d'accent).
    `<button class="sift-volume-mute" title="Couper / rétablir le son" aria-label="Couper / rétablir le son"><i class="ti ti-volume"></i></button>` +
    `<div class="sift-volume" role="slider" tabindex="0" aria-label="Volume" aria-valuemin="0" aria-valuemax="100" aria-valuenow="100">` +
    `<div class="sift-volume-track"></div>` +
    `<div class="sift-volume-fill"></div>` +
    `<div class="sift-volume-knob"></div>` +
    `</div>` +
    `</div>`
  );
}
