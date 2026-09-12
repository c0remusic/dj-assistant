// Graphique d'occupation par format — un segment par extension, trié du plus gros au plus petit.
// Un seul composant pour deux écrans : Clé USB (en-tête, informations disque, éjection) et
// Bibliothèque (barre et détail seuls, sans volume donc sans espace libre). Les options décident,
// pas deux copies qui divergeront.
//
// Tout ce qui vient du disque passe par `esc()` : un nom de volume et une extension sont des
// données utilisateur.
import type { UsageReport, ExtUsage } from "./ipc";
import { esc } from "./dom";

/** Un format, une couleur système Apple. Ces tokens `-solid` n'ont qu'un emploi — l'aplat de
 * donnée — et ne doivent jamais porter de texte (voir docs/design-system-states.md). */
const FORMAT_TOKEN: Record<string, string> = {
  ".wav": "--color-hue-blue-solid",
  ".aiff": "--color-hue-indigo-solid",
  ".aif": "--color-hue-indigo-solid",
  ".flac": "--color-hue-teal-solid",
  ".alac": "--color-hue-green-solid",
  ".mp3": "--color-hue-orange-solid",
  ".m4a": "--color-hue-yellow-solid",
  ".aac": "--color-hue-yellow-solid",
  "PIONEER/": "--color-hue-purple-solid",
  ".jpg": "--color-hue-pink-solid",
  ".jpeg": "--color-hue-pink-solid",
  ".png": "--color-hue-pink-solid",
};

/** Un format inconnu retombe sur le gris neutre plutôt que d'emprunter la couleur d'un autre :
 * deux formats de la même couleur mentiraient sur la lecture de la barre. */
export function colorFor(ext: string): string {
  return `var(${FORMAT_TOKEN[ext.toLowerCase()] ?? FORMAT_TOKEN[ext] ?? "--color-hue-gray-solid"})`;
}

/** Les groupes du détail. De la structure de format, jamais un verdict d'analyse : un fichier
 * n'est FAKE que si Sift l'a analysé, et une clé jamais passée en Revue rendrait la catégorie
 * vide sans explication — ou pire, fausse si elle ne l'est qu'à moitié. */
const GROUPS: ReadonlyArray<{ label: string; exts: readonly string[] }> = [
  { label: "Audio sans perte", exts: [".wav", ".aiff", ".aif", ".flac", ".alac"] },
  { label: "Audio compressé", exts: [".mp3", ".m4a", ".aac"] },
  { label: "Données Rekordbox", exts: ["PIONEER/"] },
];

const GO = 1_000_000_000;
export const formatGo = (bytes: number): string =>
  `${(bytes / GO).toFixed(1).replace(".", ",")} Go`;

/** Répartit les seaux dans les groupes du détail, et rassemble le reste sous « Autres fichiers ».
 * Exportée pour être testable : c'est la seule logique non triviale de ce fichier. */
export function groupBuckets(
  buckets: readonly ExtUsage[],
): Array<{ label: string; rows: ExtUsage[] }> {
  const claimed = new Set<string>();
  const out: Array<{ label: string; rows: ExtUsage[] }> = [];
  for (const g of GROUPS) {
    const rows = buckets.filter((b) => g.exts.includes(b.ext.toLowerCase()) || g.exts.includes(b.ext));
    rows.forEach((r) => claimed.add(r.ext));
    if (rows.length) out.push({ label: g.label, rows });
  }
  const rest = buckets.filter((b) => !claimed.has(b.ext));
  if (rest.length) out.push({ label: "Autres fichiers", rows: rest });
  return out;
}

export interface UsageChartOptions {
  report: UsageReport;
  /** Titre de l'en-tête. Absent = pas d'en-tête du tout (cas Bibliothèque). */
  title?: string;
  subtitle?: string;
  /** Paires de l'encadré d'informations. Le troisième élément met la valeur en alerte. */
  info?: ReadonlyArray<readonly [string, string, ("warn" | undefined)?]>;
  /** Fourni = un bouton Éjecter apparaît. Doit rejeter pour signaler un échec. */
  onEject?: () => Promise<void>;
  /** Fourni = un bouton Actualiser apparaît, à côté de l'âge de la mesure. */
  onRefresh?: () => Promise<void>;
  /** Sans carte ni inset : le graphique posé au sol d'une zone C qui ne peint rien (Clé USB depuis
   * le 2026-09-09). L'inspecteur de Rangés garde la carte. */
  plain?: boolean;
}

/** Construit la carte. Rien n'est reconstruit ensuite : le dépliage ne touche qu'une classe et le
 * surlignage qu'une autre — un `innerHTML =` à chaque clic n'animerait rien. */
export function renderUsageChart(opts: UsageChartOptions): HTMLElement {
  const { report } = opts;
  const card = document.createElement("div");
  card.className = opts.plain ? "sift-usage-card sift-usage-card-plain" : "sift-usage-card sift-ui-card-soft";

  const used = report.buckets.reduce((s, b) => s + b.bytes, 0);
  // Une bibliothèque n'est pas un volume : `free_bytes` y vaut 0 et il n'y a pas de segment libre
  // à dessiner. Le total affiché est alors la somme des formats, pas une capacité.
  const isVolume = report.free_bytes > 0;
  const total = isVolume ? report.total_bytes : used;

  if (opts.title) {
    const head = document.createElement("div");
    head.className = "sift-usage-head";
    head.innerHTML =
      DRIVE_GLYPH +
      '<div class="sift-usage-ident">' +
      `<span class="sift-usage-name">${esc(opts.title)}</span>` +
      (opts.subtitle ? `<span class="sift-usage-sub">${esc(opts.subtitle)}</span>` : "") +
      "</div>" +
      `<div class="sift-usage-capacity">${formatGo(total)}</div>`;
    card.appendChild(head);
    const rule = document.createElement("div");
    rule.className = "sift-usage-rule";
    card.appendChild(rule);
  }

  // ---- Barre + infobulle ----
  const barwrap = document.createElement("div");
  barwrap.className = "sift-usage-barwrap";
  const tip = document.createElement("div");
  tip.className = "sift-usage-tip";
  const bar = document.createElement("div");
  bar.className = "sift-usage-bar";
  const segs: HTMLElement[] = [];

  for (const b of report.buckets) {
    const pct = total > 0 ? (b.bytes / total) * 100 : 0;
    const seg = document.createElement("button");
    seg.type = "button";
    seg.className = "sift-usage-seg";
    seg.style.flex = `0 0 ${pct.toFixed(2)}%`;
    seg.style.background = colorFor(b.ext);
    seg.setAttribute(
      "aria-label",
      `${b.ext}, ${formatGo(b.bytes)}, ${b.file_count} fichiers, ${pct.toFixed(1)} % du disque`,
    );
    const show = () => {
      bar.classList.add("dim");
      segs.forEach((s) => s.classList.remove("on"));
      seg.classList.add("on");
      tip.innerHTML =
        `<span class="sift-usage-tip-ext">${esc(b.ext)}</span> — ${formatGo(b.bytes)}` +
        `<br><span class="sift-usage-tip-meta">${b.file_count} fichiers · ${pct.toFixed(1)} %</span>`;
      tip.classList.add("on");
      // Borné aux bords de la barre, sinon l'infobulle d'un segment d'extrémité déborde.
      const bw = bar.getBoundingClientRect();
      const sr = seg.getBoundingClientRect();
      tip.style.left = "0px";
      const tw = tip.offsetWidth;
      tip.style.left = `${Math.max(0, Math.min(sr.left - bw.left + sr.width / 2 - tw / 2, bw.width - tw))}px`;
    };
    const hide = () => {
      bar.classList.remove("dim");
      seg.classList.remove("on");
      tip.classList.remove("on");
    };
    seg.addEventListener("mouseenter", show);
    seg.addEventListener("focus", show);
    seg.addEventListener("mouseleave", hide);
    seg.addEventListener("blur", hide);
    seg.addEventListener("click", () => revealRow(b.ext));
    segs.push(seg);
    bar.appendChild(seg);
  }
  if (isVolume) {
    const free = document.createElement("div");
    free.className = "sift-usage-seg sift-usage-seg-free";
    free.setAttribute("aria-label", `Libre, ${formatGo(report.free_bytes)}`);
    bar.appendChild(free);
  }
  barwrap.append(tip, bar);
  card.appendChild(barwrap);

  // ---- Légende ----
  const legend = document.createElement("div");
  legend.className = "sift-usage-legend";
  for (const b of report.buckets) {
    const item = document.createElement("button");
    item.type = "button";
    item.className = "sift-usage-lg";
    item.innerHTML =
      `<span class="sift-usage-lg-top"><span class="sift-usage-swatch" style="background:${colorFor(b.ext)}"></span>` +
      `<span class="sift-usage-lg-name">${esc(b.ext)}</span></span>` +
      `<span class="sift-usage-lg-size">${formatGo(b.bytes)}</span>`;
    item.addEventListener("click", () => revealRow(b.ext));
    legend.appendChild(item);
  }
  if (isVolume) {
    const free = document.createElement("span");
    free.className = "sift-usage-lg sift-usage-lg-static";
    free.innerHTML =
      '<span class="sift-usage-lg-top"><span class="sift-usage-swatch sift-usage-swatch-free"></span>' +
      '<span class="sift-usage-lg-name sift-usage-lg-name-plain">Libre</span></span>' +
      `<span class="sift-usage-lg-size">${formatGo(report.free_bytes)}</span>`;
    legend.appendChild(free);
  }
  card.appendChild(legend);

  // ---- Encadré d'informations ----
  if (opts.info?.length) {
    const info = document.createElement("dl");
    info.className = "sift-usage-info";
    info.innerHTML = opts.info
      .map(
        ([k, v, cls]) =>
          `<div class="sift-usage-pair"><dt>${esc(k)}</dt>` +
          `<dd${cls === "warn" ? ' class="warn"' : ""}>${esc(v)}</dd></div>`,
      )
      .join("");
    card.appendChild(info);
  }

  // ---- Actions ----
  const actions = document.createElement("div");
  actions.className = "sift-usage-actions";

  const toggle = document.createElement("button");
  toggle.type = "button";
  toggle.className = "sift-usage-disclose";
  toggle.setAttribute("aria-expanded", "false");
  toggle.innerHTML =
    '<span class="sift-usage-chev">▶</span><span class="sift-usage-disclose-label">Voir le détail complet</span>';
  actions.appendChild(toggle);

  const status = document.createElement("div");
  status.className = "sift-usage-status";
  status.setAttribute("role", "status");
  status.hidden = true;

  if (opts.onRefresh) {
    const refresh = document.createElement("button");
    refresh.type = "button";
    refresh.className = "sift-usage-btn";
    refresh.textContent = "Relire le disque";
    refresh.addEventListener("click", () => {
      refresh.disabled = true;
      refresh.textContent = "Lecture…";
      void opts.onRefresh?.().catch((e: unknown) => {
        refresh.disabled = false;
        refresh.textContent = "Relire le disque";
        console.error("usage refresh failed", e);
        status.textContent = "Impossible de relire ce disque.";
        status.hidden = false;
      });
    });
    actions.appendChild(refresh);
  }

  if (opts.onEject) {
    const eject = document.createElement("button");
    eject.type = "button";
    eject.className = "sift-usage-btn";
    eject.textContent = "Éjecter";
    eject.addEventListener("click", () => {
      eject.disabled = true;
      eject.textContent = "Éjection…";
      status.hidden = true;
      status.textContent = "";
      void opts.onEject?.().catch((e: unknown) => {
        eject.disabled = false;
        eject.textContent = "Éjecter";
        console.error("ejectDrive failed", e);
        status.textContent = humanizeEject(String(e));
        status.hidden = false;
      });
    });
    actions.appendChild(eject);
  }

  card.append(actions, status);

  // ---- Détail dépliable ----
  const panel = document.createElement("div");
  panel.className = "sift-usage-panel";
  const clip = document.createElement("div");
  clip.className = "sift-usage-panel-clip";
  const inner = document.createElement("div");
  inner.className = "sift-usage-panel-inner";

  const biggest = report.buckets[0]?.bytes ?? 1;
  for (const g of groupBuckets(report.buckets)) {
    const grp = document.createElement("div");
    grp.className = "sift-usage-grp";
    grp.innerHTML =
      `<div class="sift-usage-grp-head">${esc(g.label)}` +
      `<span class="sift-usage-grp-total">${formatGo(g.rows.reduce((s, r) => s + r.bytes, 0))}</span></div>` +
      g.rows
        .map(
          (r) =>
            `<div class="sift-usage-prow" data-ext="${esc(slug(r.ext))}">` +
            `<span class="sift-usage-pext"><span class="sift-usage-swatch" style="background:${colorFor(r.ext)}"></span>${esc(r.ext)}</span>` +
            `<span class="sift-usage-ptrack"><span class="sift-usage-pfill" style="width:${((r.bytes / biggest) * 100).toFixed(1)}%;background:${colorFor(r.ext)}"></span></span>` +
            `<span class="sift-usage-pcount">${r.file_count}</span>` +
            `<span class="sift-usage-psize">${formatGo(r.bytes)}</span>` +
            "</div>",
        )
        .join("");
    inner.appendChild(grp);
  }
  clip.appendChild(inner);
  panel.appendChild(clip);
  card.appendChild(panel);

  let open = false;
  function setOpen(v: boolean): void {
    open = v;
    panel.classList.toggle("on", v);
    toggle.setAttribute("aria-expanded", String(v));
    const label = toggle.querySelector(".sift-usage-disclose-label");
    if (label) label.textContent = v ? "Masquer le détail" : "Voir le détail complet";
    // Replié, le contenu sort de l'ordre de tabulation : sinon le focus part dans des lignes de
    // hauteur nulle.
    inner.inert = !v;
  }
  function revealRow(ext: string): void {
    if (!open) setOpen(true);
    const row = inner.querySelector<HTMLElement>(`[data-ext="${slug(ext)}"]`);
    if (!row) return;
    row.classList.remove("flash");
    // Reflow forcé : retirer puis remettre la classe dans le même frame ne relance pas
    // l'animation, donc recliquer le même segment ne ferait rien.
    void row.offsetWidth;
    row.classList.add("flash");
  }
  toggle.addEventListener("click", () => setOpen(!open));
  setOpen(false);

  return card;
}

const slug = (s: string): string => s.replace(/[^a-z0-9]/gi, "");

/** Le refus du système est le cas fréquent : le message doit dire quoi fermer, pas « réessaie ». */
/** Le motif d'un refus d'éjection, en français et en entier (spec cle-usb.md § États :
 * « nommer ce qui tient le volume »). Exportée pour `usb-view.ts`, qui porte ses propres boutons
 * depuis le 2026-09-09. */
export function humanizeEject(raw: string): string {
  if (raw.includes("EJECT_BUSY")) {
    return (
      "Windows refuse de démonter ce disque : un programme le tient encore ouvert. " +
      "Ferme Rekordbox et les fenêtres de l'explorateur, puis réessaie. Rien n'a été démonté — " +
      "ne le débranche pas en l'état."
    );
  }
  if (raw.includes("DRIVE_VANISHED")) return "Ce disque n'est déjà plus branché.";
  return "Éjection impossible.";
}

/** Icône de disque externe, dessinée en ligne : la CSP interdit toute ressource distante et une
 * icône de police ne se colore pas par zone. */
const DRIVE_GLYPH = `
<svg class="sift-usage-glyph" viewBox="0 0 56 56" aria-hidden="true">
  <rect x="6" y="12" width="44" height="32" rx="5" fill="var(--color-text-tertiary)" opacity="0.18"/>
  <rect x="6" y="12" width="44" height="32" rx="5" fill="none" stroke="var(--color-text-tertiary)" stroke-width="1.25" opacity="0.55"/>
  <rect x="12" y="18" width="32" height="9" rx="2.5" fill="var(--color-text-tertiary)" opacity="0.35"/>
  <circle cx="16" cy="37" r="2.4" fill="var(--color-hue-green-solid)"/>
  <rect x="23" y="35.2" width="21" height="3.6" rx="1.8" fill="var(--color-text-tertiary)" opacity="0.3"/>
</svg>`;
