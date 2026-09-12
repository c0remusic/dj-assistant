import type { Meta, StoryObj } from "@storybook/html-vite";
import { playerAuditionHtml, volumeCentreCss, volumeIconClass } from "./player-audition";

// La rangée d'audition du lecteur de Revue (`player-audition.ts`) : play 28/22, slider de
// progression du kit (piste 4 px `--overlay-bar`, remplissage `--color-accent-fill`, pouce blanc
// 20), temps unique cliquable, volume fin 90 px (remplissage et pouce `--color-accent-ink`,
// pouce 14). Catalogué dans `design-system-states.md` § « Lecteur simple ». Ces stories EXÉCUTENT
// le vrai markup (`playerAuditionHtml`, module pur sans `./ipc`) puis lui appliquent les MÊMES
// mutations que `mountPlayer` (report-view.ts) : remplissage/pouce en `%` côté progression,
// `volumeCentreCss` côté volume, `volumeIconClass` pour l'icône de mute — les formules sont
// importées, pas recopiées.
//
// NON REPRÉSENTABLE ICI : l'anneau `:focus-visible` (vrai geste clavier exigé), le seek au drag
// (moteur WaveSurfer absent — son conteneur `.sift-progress-engine` reste un carré vide de 0 px,
// exactement comme en prod), et le `:hover` du temps (`.sift-time:hover`), injecté au runtime par
// `ensureStyles()` (report-view.ts) et non par `styles.css`.
interface AuditionArgs {
  /** Position de lecture, 0–100. */
  progress: number;
  /** Volume, 0–100 (0 = muet, icône barrée). */
  volume: number;
  /** Texte du temps (posé par `updateTime` en prod — donnée d'affichage, pas une formule). */
  time: string;
  /** Montre la bulle mm:ss de survol (patron QuickTime), à la position de lecture. */
  hoverBubble: boolean;
}

function audition({ progress, volume, time, hoverBubble }: AuditionArgs): HTMLElement {
  // Le cadre de lecture réel : `.sift-player-row` porte la surface (fond queue, rayon md — le
  // « cadre Y » validé le 2026-08-27) — la rangée se juge sur lui.
  const host = document.createElement("div");
  host.className = "sift-player-row";
  host.style.maxWidth = "560px";
  host.innerHTML = playerAuditionHtml();

  const pct = Math.max(0, Math.min(100, progress));
  const fill = host.querySelector<HTMLElement>(".sift-progress-fill");
  const knob = host.querySelector<HTMLElement>(".sift-progress-knob");
  if (fill) fill.style.width = `${pct}%`;
  if (knob) {
    // Même contrat que `updateTime` : le pouce est révélé dès que la durée est connue.
    knob.hidden = false;
    knob.style.left = `${pct}%`;
  }
  host.querySelector(".sift-progress")?.setAttribute("aria-valuenow", String(Math.round(pct)));

  const timeEl = host.querySelector<HTMLElement>(".sift-time");
  if (timeEl) timeEl.textContent = time;

  const vol = Math.max(0, Math.min(1, volume / 100));
  const centre = volumeCentreCss(vol);
  const vFill = host.querySelector<HTMLElement>(".sift-volume-fill");
  const vKnob = host.querySelector<HTMLElement>(".sift-volume-knob");
  if (vFill) vFill.style.width = centre;
  if (vKnob) vKnob.style.left = centre;
  host.querySelector(".sift-volume")?.setAttribute("aria-valuenow", String(Math.round(vol * 100)));
  const icon = host.querySelector<HTMLElement>(".sift-volume-mute i");
  if (icon) icon.className = volumeIconClass(vol);

  if (hoverBubble) {
    const bubble = host.querySelector<HTMLElement>(".sift-wave-hovertime");
    if (bubble) {
      bubble.hidden = false;
      bubble.style.left = `${pct}%`;
      bubble.textContent = time;
    }
  }
  return host;
}

const meta: Meta<AuditionArgs> = {
  title: "États de contenu/Revue — lecteur simple",
  render: audition,
  argTypes: {
    progress: { control: { type: "range", min: 0, max: 100 } },
    volume: { control: { type: "range", min: 0, max: 100 } },
    time: { control: "text" },
    hoverBubble: { control: "boolean" },
  },
  args: { progress: 37, volume: 100, time: "1:23", hoverBubble: false },
};

export default meta;
type Story = StoryObj<AuditionArgs>;

/** Lecture en cours : remplissage accent, pouce blanc 20 révélé, volume plein. */
export const EnLecture: Story = {};

/** Avant que la durée soit connue : remplissage à 0, pouce re-caché — c'est la coquille initiale
 *  du markup (`hidden`), que `updateTime` ne lève qu'une fois `dur > 0`. Les autres stories
 *  figurent toutes une durée connue. */
export const DureeInconnue: Story = {
  render: (args) => {
    const host = audition({ ...args, progress: 0 });
    const knob = host.querySelector<HTMLElement>(".sift-progress-knob");
    if (knob) knob.hidden = true;
    return host;
  },
  args: { progress: 0, time: "0:00" },
};

/** Survol de la piste : bulle mm:ss (`--overlay-scrim` / `--color-text-on-scrim`) au-dessus de la
 *  position pointée — seule survivante du survol de l'ex-waveform. */
export const SurvolBulle: Story = { args: { hoverBubble: true } };

/** Muet : remplissage et pouce du volume à 0, glyphe barré (`ti-volume-off`) — l'état se dit par
 *  l'ICÔNE, plus par un slash permanent comme sur l'ex-capsule. */
export const Muet: Story = { args: { volume: 0 } };

/** Temps en mode restant (clic ou Entrée/Espace sur `.sift-time` en prod) : décompte préfixé `-`. */
export const TempsRestant: Story = { args: { time: "-2:41" } };
