import type { Meta, StoryObj } from "@storybook/html-vite";

// Les neuf teintes `-solid` ajoutées le 2026-08-01 pour les surfaces de donnée (segments de
// graphique d'occupation disque), plus `red` le 2026-08-27 (systemRed du kit, ajouté pour la
// pastille de verdict Faux de la file — voir `queue-verdict-dot.ts`). Lues DEPUIS les tokens de
// styles.css, jamais recopiées : une story qui redéclare ses couleurs cesse d'être un miroir dès
// la première retouche. Ordre = celui du bloc `:root`.
const HUES = ["blue", "indigo", "teal", "green", "orange", "yellow", "purple", "pink", "red", "gray"];

/** Les cinq teintes historiques ont trois variantes, les quatre nouvelles n'ont que `-solid` —
 * `-bg`/`-text` n'ont de sens que pour une puce, et rien n'en demande pour l'instant. */
const HAS_CHIP_VARIANTS = new Set(["indigo", "teal", "purple", "pink", "yellow"]);

function renderPalette(): HTMLElement {
  const wrap = document.createElement("div");
  wrap.style.cssText =
    "display:flex;flex-direction:column;gap:var(--space-16);padding:var(--space-16);" +
    "background:var(--color-background-primary);color:var(--color-text-primary);" +
    "font-family:var(--font-ui);font-size:var(--text-base)";

  // Une barre continue : c'est l'usage réel, et c'est là qu'on voit si deux teintes se confondent.
  const bar = document.createElement("div");
  bar.style.cssText =
    "display:flex;height:26px;border-radius:var(--border-radius-sm);overflow:hidden";
  for (const h of HUES) {
    const seg = document.createElement("div");
    seg.style.cssText = `flex:1;background:var(--color-hue-${h}-solid)`;
    bar.appendChild(seg);
  }
  wrap.appendChild(bar);

  const grid = document.createElement("div");
  grid.style.cssText =
    "display:grid;grid-template-columns:repeat(auto-fill,minmax(190px,1fr));gap:var(--space-12)";
  for (const h of HUES) {
    const row = document.createElement("div");
    row.style.cssText = "display:flex;align-items:center;gap:var(--space-8);min-width:0";
    const sw = document.createElement("span");
    // Carré arrondi, pas une pastille : forme retenue pour les légendes de graphique.
    sw.style.cssText =
      `width:14px;height:14px;border-radius:3px;flex:none;background:var(--color-hue-${h}-solid)`;
    const name = document.createElement("span");
    name.style.cssText =
      "font-family:var(--font-mono);font-size:var(--text-sm);overflow:hidden;" +
      "text-overflow:ellipsis;white-space:nowrap";
    name.textContent = `--color-hue-${h}-solid`;
    const tag = document.createElement("span");
    tag.style.cssText =
      "margin-left:auto;font-size:var(--text-xs);color:var(--color-text-tertiary);flex:none";
    tag.textContent = HAS_CHIP_VARIANTS.has(h) ? "+ bg/text" : "solid seul";
    row.append(sw, name, tag);
    grid.appendChild(row);
  }
  wrap.appendChild(grid);
  return wrap;
}

const meta: Meta = {
  title: "Fondations/Teintes pleines",
  render: renderPalette,
};

export default meta;
type Story = StoryObj;

/** À basculer en sombre via la barre d'outils Storybook : Apple publie deux jeux, et les deux
 * sont dans `styles.css`. Réutiliser les valeurs claires sur fond sombre les ferait plonger. */
export const Palette: Story = {};
