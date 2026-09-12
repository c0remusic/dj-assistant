import type { Meta, StoryObj } from "@storybook/html-vite";
import type { Source } from "../shared/contracts";
import { sourceEntryHtml } from "./rail-source-entry";

// Ligne de source du rail (`rail-source-entry.ts`, section Sources — fusion 1) : les états réels
// de la pastille et de la ligne, catalogués dans `design-system-states.md` § « Ligne de source du
// rail ». Ces stories EXÉCUTENT le vrai rendu (`sourceEntryHtml`, module pur sans `./ipc`) au lieu
// d'en recopier le markup — modèle `journal-table.stories.ts` : une copie ne peut que diverger.
//
// Le conteneur reprend la vraie charpente du rail (`.sb`, `index.html:16`) : les pastilles se
// jugent sur la teinte qui les porte en prod — `--color-background-primary` depuis le 2026-08-27
// (rail EN RETRAIT, plan le plus bas des trois : rail < milieu < file, motif sidebar Mail ; le
// chrome `--color-background-tertiary` d'avant a suivi la décision) — pas sur le fond du canvas.
// Largeur `--rail-w` comprise — c'est elle qui donne leur mesure aux libellés.
//
// ÉTAT NON REPRÉSENTABLE ICI : le rail replié (`body.sift-rail-collapsed`, pastille 14px, contour
// suspendu 1.5px) — la classe vit sur <body>, hors de portée d'une story statique.
interface RailSrcArgs {
  /** Compte de nouveaux fichiers de la 1re ligne (0 = pas de badge). */
  pending: number;
}

function src(id: number, path: string, over: Partial<Source> = {}): Source {
  return { id, path, pending_count: 0, track_count: 12, accessible: true, watched: true, color_key: null, ...over };
}

function railHost(inner: string): HTMLElement {
  const host = document.createElement("div");
  host.className = "sb";
  // Cadran de dépôt (issue #56, décision A) : la section vit dans un cadre hairline permanent en
  // bas du rail — la story reprend la classe réelle (`.sift-rail-drop`, aussi posée sur
  // `#sift-rail-sources` dans index.html) pour montrer le cadre.
  host.innerHTML = `<div class="sift-rail-drop"><div class="nv-grp">Sources</div>${inner}</div>`;
  return host;
}

/** Les cinq teintes du cycle, par ordre d'ajout — pastille pleine, l'accent catégoriel au repos. */
export const TeintesDuCycle: StoryObj<RailSrcArgs> = {
  render: ({ pending }) => {
    const all = [
      src(1, "C:\\music\\incoming", { pending_count: pending }),
      src(2, "C:\\music\\promos"),
      src(3, "C:\\music\\bandcamp"),
      src(4, "C:\\music\\rips"),
      src(5, "C:\\music\\edits"),
    ];
    return railHost(all.map((s) => sourceEntryHtml(s, all, s.id === 2, undefined)).join(""));
  },
  args: { pending: 8 },
};

/** Échec de scan et dossier inaccessible : encre `danger` sur la ligne, motif dans l'infobulle.
 *  Jamais atténués — un échec se voit mieux que le reste (rail.md § États). */
export const ScanEchoue: StoryObj<RailSrcArgs> = {
  render: () => {
    const all = [
      src(1, "C:\\music\\incoming"),
      src(2, "C:\\music\\promos"),
      src(3, "D:\\usb\\imports", { accessible: false }),
    ];
    return railHost(
      [
        sourceEntryHtml(all[0], all, false, undefined),
        sourceEntryHtml(all[1], all, false, "dossier verrouillé par un autre processus"),
        sourceEntryHtml(all[2], all, false, undefined),
      ].join(""),
    );
  },
};

/** Surveillance suspendue : pastille VIDÉE — contour sans fond, teinte conservée (elle identifie
 *  la source, DESIGN.md § 4). L'encre de ligne ne change pas : état permanent donc neutre. */
export const SurveillanceSuspendue: StoryObj<RailSrcArgs> = {
  render: () => {
    const all = [
      src(1, "C:\\music\\incoming"),
      src(2, "C:\\music\\promos", { watched: false }),
      src(3, "C:\\music\\bandcamp", { watched: false, color_key: "yellow" }),
    ];
    return railHost(all.map((s) => sourceEntryHtml(s, all, false, undefined)).join(""));
  },
};

/** Aucun fichier audio reconnu (`track_count === 0`, issue #55, décision B2) : badge « 0 audio »
 *  en encre warning, pastille éteinte (opacité réduite, teinte conservée). Précédence :
 *  échec > vide > suspension — la 3e ligne, suspendue ET vide, rend le badge quand même. */
export const AucunFichierReconnu: StoryObj<RailSrcArgs> = {
  render: () => {
    const all = [
      src(1, "C:\\music\\incoming"),
      src(2, "C:\\music\\pochettes", { track_count: 0 }),
      src(3, "C:\\music\\cuesheets", { track_count: 0, watched: false }),
    ];
    return railHost(all.map((s) => sourceEntryHtml(s, all, false, undefined)).join(""));
  },
};

const meta: Meta<RailSrcArgs> = {
  title: "Composants/Ligne de source du rail",
};

export default meta;
