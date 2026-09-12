// Live "Clé USB" screen (Tauri only) — its own nav destination since 2026-07-31. It used to be a
// card inside Réglages (#sift-reglages-usb) with the nav item redirecting there (finding F5,
// audit-heuristique-visuel.md), so the "Clé USB" item lit up "Réglages" and led to a page about
// something else. Everything USB-related now lives here; Réglages no longer carries any of it.
//
// Lu contre Utilitaire de disque le 2026-09-09 (déclinaison #24, sixième écran — v3 validée par
// Antoine, « go v3 » ; `docs/ui-specs/cle-usb.md` § Décision). La référence a trois organes et
// l'écran les reprend tels quels : une SIDEBAR des appareils (colonne B′, au plan de la file, même
// grammaire que la colonne des sections de Rekordbox), une ZONE PRINCIPALE pour l'appareil choisi
// (tête icône + nom + capacité encadrée, barre d'occupation, grille de faits sur quatre colonnes),
// et l'action destructrice derrière une SHEET (`usb-format-modal.ts`). Les actions vivent sous les
// faits, dans la section (grammaire Finder › appareil, comme Rekordbox) — la barre ne porte
// qu'Actualiser, et le disque est la zone C : pas de zone D sur cet écran.
//
// Ce que cette lecture RETIRE : la carte `.sift-settings-stack` de 560 px (44 % de la fenêtre vide,
// DESIGN.md § 11 O-3), le paragraphe d'explication en tête, la ligne « lettre · modèle · taille ·
// Formater… » et la carte d'occupation empilée SOUS chaque ligne (deux fois l'identité du disque).
import { listRemovableDrives, driveUsage, ejectDrive } from "./ipc";
import type { RemovableDrive, UsageReport } from "./ipc";
import { requireEl, esc } from "./dom";
import { openUsbFormatModal } from "./usb-format-modal";
import { usbEntryHtml, driveDisplayName } from "./usb-row";
import { renderUsageChart, formatGo, humanizeEject } from "./usage-chart";
import { mountBarActions } from "./toolbar";
import { emptyStateHtml } from "./empty-state";
import { openContextMenu } from "./context-menu";
import { viewEpoch, isStaleViewRender } from "./view-epoch";

/** Holds the currently-attached `sift:usb-format-done` window listener, if any, so `renderUsbLive()`
 * can remove it before attaching a new one. Without this, every re-render of the screen (each nav
 * visit) piles up another listener on `window` — unlike DOM nodes, a `window` listener has no parent
 * to disappear with, so it accumulates forever. */
let usbFormatDoneHandler: (() => void) | null = null;

/** Dernière énumération, et le disque choisi dans la colonne. `activeId` survit à un re-rendu
 * (retour sur l'écran, Actualiser) tant que le disque est encore branché — sinon le premier. */
let drives: RemovableDrive[] = [];
let activeId: string | null = null;

const SKELETON =
  `<div class="sift-usb-layout"><nav class="sift-usb-side"><div class="col-h">Disques amovibles</div>` +
  `<span class="sift-skel sift-skel-line"></span></nav>` +
  `<div class="sift-usb-main"><span class="sift-skel sift-skel-line"></span></div></div>`;

/** Live Clé USB view. Renders the whole page fresh each call, same pattern as
 *  renderRekordboxLive — no mock DOM survives. */
export function renderUsbLive(): void {
  const content = requireEl("#content", "renderUsbLive");
  // Squelette statique au premier passage (DESIGN.md § 6). Un re-rendu garde l'écran précédent.
  if (!content.querySelector(".sift-usb-layout, .sift-empty-state")) content.innerHTML = SKELETON;
  mountBar();
  if (usbFormatDoneHandler) window.removeEventListener("sift:usb-format-done", usbFormatDoneHandler);
  usbFormatDoneHandler = () => void reload();
  window.addEventListener("sift:usb-format-done", usbFormatDoneHandler);
  void reload();
}

/** Barre unifiée : Actualiser seul. Utilitaire de disque n'a pas ce bouton — ses appareils
 * apparaissent d'eux-mêmes — mais l'énumération WMI de Sift se fait à la demande, et un lecteur de
 * cartes qu'on vient de remplir ne se signale pas tout seul. */
function mountBar(): void {
  mountBarActions(`<button id="sift-usb-refresh" class="sift-bar-btn" type="button">Actualiser</button>`);
  document.getElementById("sift-usb-refresh")?.addEventListener("click", () => void reload());
}

function setCount(text: string): void {
  const el = document.getElementById("sift-tb-count");
  if (el) el.textContent = text;
}

/** Ré-énumère, puis repeint. La colonne garde le disque choisi s'il est toujours là. */
async function reload(): Promise<void> {
  const content = requireEl("#content", "renderUsbLive");
  // Jeton capturé dans le même geste que `#content` (issue #42) : l'énumération WMI prend des
  // secondes, et l'utilisateur peut avoir changé d'écran entre-temps.
  const token = viewEpoch();
  try {
    drives = await listRemovableDrives();
  } catch (e) {
    console.error("listRemovableDrives failed", e);
    if (isStaleViewRender(token)) return;
    drives = [];
    setCount("");
    // The raw chain, not a generic sentence: an enumeration failure here is a backend fault and
    // hiding it is what let a broken WMI query look like "no drive plugged in" for months
    // (CLAUDE.md § Méthode — pas de fallback silencieux).
    content.innerHTML =
      `<div class="sift-usb-empty sift-usb-danger">Impossible de lister les disques amovibles.<br>${esc(String(e))}</div>`;
    return;
  }
  if (isStaleViewRender(token)) return;
  if (!drives.length) {
    activeId = null;
    setCount("");
    // Naming the most common false alarm: an empty card-reader slot keeps its drive letter in
    // the Explorer sidebar forever, so "je vois E: dans l'explorateur" is not evidence that
    // anything is plugged in.
    content.innerHTML = emptyStateHtml({
      title: "Aucun disque amovible détecté",
      note:
        "Un lecteur de cartes vide garde sa lettre dans l'explorateur Windows sans qu'aucune clé ne " +
        "soit branchée — vérifie que la clé est bien enfoncée, puis Actualiser. Seuls les disques " +
        "amovibles sont proposés : aucun disque interne n'apparaît ici.",
      actionHtml: `<button type="button" id="sift-usb-refresh-empty">Actualiser</button>`,
    });
    content.querySelector("#sift-usb-refresh-empty")?.addEventListener("click", () => void reload());
    return;
  }
  if (!drives.some((d) => d.id === activeId)) activeId = drives[0].id;
  setCount(`${drives.length} disque${drives.length > 1 ? "s" : ""}`);
  paint(content);
}

/** Repeint les deux zones depuis `drives` + `activeId`. Appelée à chaque changement de disque :
 * l'écran a au plus une poignée d'entrées, reconstruire est moins risqué que muter. */
function paint(content: HTMLElement): void {
  const cur = drives.find((d) => d.id === activeId) ?? drives[0];
  const side =
    `<nav class="sift-usb-side" aria-label="Disques amovibles"><div class="col-h">Disques amovibles</div>` +
    drives.map((d) => usbEntryHtml(d, d.id === cur.id)).join("") +
    `</nav>`;
  content.innerHTML =
    `<div class="sift-usb-layout">${side}<div class="sift-usb-main"><div class="sift-usb-main-inner">` +
    headHtml(cur) +
    `<div id="sift-usb-body"></div>` +
    `</div></div></div>`;
  for (const el of content.querySelectorAll<HTMLElement>("[data-usb-id]")) {
    const pick = () => {
      activeId = el.dataset.usbId ?? null;
      paint(content);
    };
    el.addEventListener("click", pick);
    el.addEventListener("keydown", (e) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        pick();
      }
    });
    el.addEventListener("contextmenu", (e) => {
      e.preventDefault();
      const d = drives.find((x) => x.id === el.dataset.usbId);
      if (d) openDriveMenu(e.clientX, e.clientY, d, content);
    });
  }
  content.querySelector<HTMLElement>(".sift-usb-main")?.addEventListener("contextmenu", (e) => {
    e.preventDefault();
    openDriveMenu(e.clientX, e.clientY, cur, content);
  });
  const body = requireEl("#sift-usb-body", "renderUsbLive");
  if (!cur.has_media) {
    body.innerHTML =
      `<div class="sift-usb-empty">Aucun média inséré. Ce lecteur garde sa lettre dans l'explorateur ` +
      `Windows même vide — insère une carte ou une clé, puis Actualiser.</div>`;
    return;
  }
  void mountDisk(body, cur, false);
}

/** Tête de zone C : la même `.sift-usage-head` que Rekordbox (glyphe, nom, une ligne de faits,
 * capacité encadrée). Le nom est celui du VOLUME quand il existe — c'est ce que l'utilisateur lit
 * dans l'explorateur et ce qu'il retape pour armer le formatage. */
function headHtml(d: RemovableDrive): string {
  const sub = d.has_media
    ? `Disque USB externe · ${esc(d.current_fs || "non formaté")}${d.mount ? ` · ${esc(d.mount)}` : ""} · ${esc(d.label || "Disque amovible")}`
    : `Lecteur amovible · aucun média inséré · ${esc(d.label || "Lecteur amovible")}`;
  return (
    `<div class="sift-usage-head sift-usb-head">` +
    `<span class="sift-usb-glyph" aria-hidden="true"><i class="ti ti-usb"></i></span>` +
    `<div class="sift-usage-ident"><span class="sift-usage-name">${esc(d.volume_name || driveDisplayName(d))}</span>` +
    `<span class="sift-usage-sub">${sub}</span></div>` +
    (d.has_media ? `<div class="sift-usage-capacity">${formatGo(d.size_bytes)}</div>` : "") +
    `</div><div class="sift-usage-rule"></div>`
  );
}

/** Zone C sous la tête : barre d'occupation + légende (le graphique, sans sa carte — la zone C ne
 * peint rien, Rangés et Rekordbox non plus), un filet, la grille de faits d'Utilitaire de disque,
 * puis les actions du disque. Le parcours peut prendre quelques secondes au premier passage
 * (ensuite le backend sert son cache), donc le corps annonce l'attente au lieu de rester vide —
 * un blanc se lit comme une panne. */
async function mountDisk(body: HTMLElement, d: RemovableDrive, force: boolean): Promise<void> {
  body.innerHTML = `<div class="sift-usb-empty">Analyse de l'occupation…</div>`;
  const token = viewEpoch();
  let report: UsageReport | null = null;
  let usageError: string | null = null;
  try {
    report = await driveUsage(d.id, force);
  } catch (e) {
    console.error("driveUsage failed", e);
    usageError = String(e);
  }
  if (isStaleViewRender(token) || activeId !== d.id) return;
  body.innerHTML = "";
  if (report) {
    body.appendChild(renderUsageChart({ report, plain: true }));
  } else {
    // Un disque non formaté n'a rien à parcourir : le dire, sans la chaîne brute qui reste au
    // journal. Une clé formatée dont la lecture échoue, elle, montre la cause.
    body.innerHTML = d.mount
      ? `<div class="sift-usb-empty sift-usb-danger">Occupation indisponible.<br>${esc(usageError ?? "")}</div>`
      : `<div class="sift-usb-empty">Aucun volume monté — rien à parcourir tant que le disque n'est pas formaté.</div>`;
  }
  body.insertAdjacentHTML("beforeend", `<div class="sift-usage-rule sift-usb-rule"></div>` + factsHtml(d, report) + actionsHtml(d));
  wireActions(body, d);
}

/** La grille de faits d'Utilitaire de disque : quatre colonnes, libellé au-dessus en encre
 * secondaire, valeur en dessous — alignée sur la barre, jamais une colonne de libellés à droite
 * (v2 réfutée par Antoine : « le layout est bizarre avec le texte »). Huit faits, tous portés par
 * `RemovableDrive` ou le rapport : pas de « Connexion » tant que le bus n'est pas remonté. */
function factsHtml(d: RemovableDrive, r: UsageReport | null): string {
  const cell = (k: string, v: string, cls?: "warn" | "mono"): string =>
    `<div class="sift-usb-fact"><dt>${k}</dt><dd${cls ? ` class="${cls}"` : ""}>${v}</dd></div>`;
  const healthWarn = d.health !== "" && d.health !== "OK";
  return (
    `<dl class="sift-usb-facts">` +
    cell("Point de montage", d.mount ? esc(d.mount) : "—") +
    cell("Format", esc(d.current_fs || "non formaté")) +
    cell("Capacité", formatGo(d.size_bytes)) +
    cell("Libre", d.mount ? formatGo(d.free_bytes) : "—") +
    cell("Fichiers", r ? String(r.file_count) : "—") +
    cell("Modèle", esc(d.label || "—")) +
    cell("Périphérique", esc(d.id.replace(/^\\\\\.\\/, "")), "mono") +
    // Non OK = mis en alerte. C'est le seul fait de la grille qui appelle une action de la part
    // d'Antoine, il ne doit pas se fondre dans les autres.
    cell("Santé", esc(d.health || "—"), healthWarn ? "warn" : undefined) +
    `</dl>`
  );
}

/** Les actions du disque, sous les faits (Finder › appareil : les actions secondaires dans la
 * section). Formater… ouvre la sheet ; Éjecter agit tout de suite et dit son échec sur place. */
function actionsHtml(d: RemovableDrive): string {
  return (
    `<div class="sift-usb-actions">` +
    `<button type="button" class="sift-usage-btn" data-usb-act="format">Formater…</button>` +
    (d.mount ? `<button type="button" class="sift-usage-btn" data-usb-act="eject">Éjecter</button>` : "") +
    (d.mount ? `<button type="button" class="sift-usage-btn" data-usb-act="reread">Relire le disque</button>` : "") +
    `</div><div class="sift-usage-status" role="status" hidden></div>`
  );
}

function wireActions(body: HTMLElement, d: RemovableDrive): void {
  const status = body.querySelector<HTMLElement>(".sift-usage-status");
  const say = (msg: string) => {
    if (!status) return;
    status.textContent = msg;
    status.hidden = false;
  };
  body.querySelector("[data-usb-act=format]")?.addEventListener("click", () => openUsbFormatModal(d));
  body.querySelector<HTMLButtonElement>("[data-usb-act=reread]")?.addEventListener("click", (e) => {
    (e.currentTarget as HTMLButtonElement).disabled = true;
    void mountDisk(body, d, true);
  });
  body.querySelector<HTMLButtonElement>("[data-usb-act=eject]")?.addEventListener("click", (e) => {
    const btn = e.currentTarget as HTMLButtonElement;
    btn.disabled = true;
    btn.textContent = "Éjection…";
    void doEject(d).catch((err: unknown) => {
      btn.disabled = false;
      btn.textContent = "Éjecter";
      say(humanizeEject(String(err)));
    });
  });
}

/** Éjecté : l'entrée comme la zone C n'ont plus d'objet. On repart d'une liste fraîche plutôt
 * que de retirer l'entrée à la main et risquer de mentir. Rejette pour signaler un échec. */
async function doEject(d: RemovableDrive): Promise<void> {
  try {
    await ejectDrive(d.id);
  } catch (e) {
    console.error("ejectDrive failed", e);
    throw e;
  }
  await reload();
}

/** Clic droit sur une entrée ou sur la zone C (spec § Interactions). « Ouvrir dans l'explorateur »
 * attend une commande IPC qui n'existe pas encore — omis plutôt qu'inventé. */
function openDriveMenu(x: number, y: number, d: RemovableDrive, content: HTMLElement): void {
  openContextMenu(x, y, [
    { label: "Formater…", onPick: d.has_media ? () => openUsbFormatModal(d) : undefined, danger: true },
    {
      label: "Éjecter",
      onPick: d.mount
        ? () => {
            void doEject(d).catch((err: unknown) => {
              const status = content.querySelector<HTMLElement>(".sift-usage-status");
              if (status) {
                status.textContent = humanizeEject(String(err));
                status.hidden = false;
              }
            });
          }
        : undefined,
    },
    { label: "Actualiser", onPick: () => void reload(), separated: true },
  ]);
}
