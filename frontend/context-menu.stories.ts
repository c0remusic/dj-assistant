import type { Meta, StoryObj } from "@storybook/html-vite";

// Menu contextuel (`context-menu.ts`, `.sift-ctx-menu`) — états réels, dont la rangée de
// pastilles de couleur d'une source (patron Finder Tags : les couleurs vivent EN RANGÉE dans le
// menu même, jamais dans un sous-menu). La story reproduit le markup exact que
// `openContextMenu()` génère : le vrai composant s'ancre en `position:fixed` au point du clic et
// se ferme au premier clic extérieur — inutilisable tel quel dans un canvas Storybook, d'où la
// reproduction statique, même approche que `segmented-control.stories.ts`.
interface CtxMenuArgs {
  /** Teinte marquée d'un anneau (la couleur RÉSOLUE de la source). */
  active: string;
  /** true = un override manuel est posé, donc « Couleur automatique » est cliquable. */
  override: boolean;
}

const HUES: readonly { key: string; label: string }[] = [
  { key: "indigo", label: "Indigo" },
  { key: "purple", label: "Violet" },
  { key: "pink", label: "Rose" },
  { key: "teal", label: "Turquoise" },
  { key: "yellow", label: "Jaune" },
];

function renderMenu({ active, override }: CtxMenuArgs): HTMLElement {
  const menu = document.createElement("div");
  menu.className = "sift-ctx-menu";
  menu.style.position = "static"; // le vrai menu est fixed au point de clic — sans objet ici
  const dots = HUES.map(
    (h) =>
      `<button type="button" class="sift-ctx-swatch${h.key === active ? " on" : ""}" title="${h.label}">` +
      `<span class="sift-ctx-swatch-fill sift-rail-src-dot-${h.key}" aria-hidden="true"></span></button>`,
  ).join("");
  menu.innerHTML =
    `<button type="button" class="sift-ctx-item">Suspendre la surveillance</button>` +
    `<button type="button" class="sift-ctx-item">Rescanner</button>` +
    `<div class="sift-ctx-swatchrow sift-ctx-item--sep"><span class="sift-ctx-swatchlabel">Couleur</span>${dots}</div>` +
    `<button type="button" class="sift-ctx-item${override ? "" : " sift-ctx-item--disabled"}">Couleur automatique</button>` +
    `<button type="button" class="sift-ctx-item sift-ctx-item--disabled sift-ctx-item--sep">Ouvrir l'emplacement</button>` +
    `<button type="button" class="sift-ctx-item sift-ctx-item--danger sift-ctx-item--sep">Retirer de la surveillance</button>`;
  return menu;
}

const meta: Meta<CtxMenuArgs> = {
  title: "Composants/Menu contextuel",
  render: renderMenu,
  argTypes: {
    active: { control: "select", options: HUES.map((h) => h.key) },
  },
};

export default meta;
type Story = StoryObj<CtxMenuArgs>;

/** Source sans override : anneau sur la teinte du cycle, « Couleur automatique » désactivée
 *  (jamais retirée — le menu garde les mêmes entrées aux mêmes positions). */
export const SourceCouleurAuto: Story = {
  args: { active: "purple", override: false },
};

/** Override posé : anneau sur la teinte choisie, « Couleur automatique » redevient cliquable. */
export const SourceCouleurOverride: Story = {
  args: { active: "teal", override: true },
};
