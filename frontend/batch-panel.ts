// Revue batch mode panel — selection summary (zone C), filing rail, batch filing.
// Extracted from sift-live.ts (Phase 1, tranche 1c). Reads currentItems/reviewMode
// from queue-panel.ts (leaf-to-leaf import, one direction only — queue-panel.ts never
// imports from here). sift-live.ts's setReviewMode imports renderBatch + batch
// destination state.
//
// Wireframe §09 : le mode Lot est un mode de SÉLECTION dans la colonne de file, pas
// un deuxième tableau. Zone C = résumé de sélection uniquement (selectionSummaryHtml).
// Le board à trois groupes (Prêts/À vérifier/En analyse) et ses sélecteurs internes
// (batchSel, batchFakeSel, groupHead, cappedBody) ont été retirés — la sélection vit
// dans queueBatchSel (queue-panel.ts), pilotée par les cases de la colonne de file.
import { openFilingInto, TARGET_LABEL } from "./filing";
import {
  renderBinsForBatch,
  ensureDestPopoverAutoClose,
  setBinPickInert,
  toggleDestPopover,
  repositionDestPopoverIfOpen,
} from "./filing-bins";
import { fileBatch, fileCancel, rejectBatch } from "./ipc";
import { requireEl, esc } from "./dom";
import { toast } from "./filing-toast";
import { openSettingsScreen } from "./filing-actions";
import { slideSegThumb } from "./seg-thumb";
import type { BatchResult, FileProgress, Target } from "../shared/contracts";
import { FILE_IN_PLACE, EXTERNAL_DEST_PREFIX } from "../shared/contracts";
import {
  setTask,
  clearTask,
  mountProgressZone,
  homeProgressZone,
} from "./progress-zone";
import {
  startBatchTracklist,
  updateBatchTracklist,
  finishBatchTracklist,
  clearBatchTracklist,
} from "./batch-tracklist";
import { currentItems, reviewMode, prefetchNextAfter, enterDetailMode, queueBatchSel } from "./queue-panel";
import { selectionSummaryHtml } from "./selection-summary";
import { confirmBatchAlert, BATCH_CONFIRM_THRESHOLD } from "./confirm-modal";
import type { BatchAlertData } from "./confirm-modal";
import { showBatchSheet, updateBatchSheet, transformToReport, closeBatchSheet } from "./batch-sheet";

/** Human label for the batch destination (resolves the in-place sentinel to its prose). */
const IN_PLACE_LABEL = "Dossier source de chaque morceau";

// Batch "file in place" toggle (FILE_IN_PLACE). Kept apart from batchBin so the picked folder is
// remembered while in-place is on. Effective destination = batchInPlace ? FILE_IN_PLACE : batchBin.
export let batchInPlace = false;
// Format de sortie du rail LOSSLESS uniquement. Le rail lossy n'a pas de choix: y demander AIFF ou
// WAV serait de l'upscale, que le backend refuse (`filing.rs`, `guard_no_upscale`).
let batchLosslessFormat: Target = "aiff_16_44";
let skipBatchConfirm = false;
// The ordered ids submitted to the currently-running batch — drives the per-track tracklist (the
// nth `file:progress.done` maps to batchTrackIds[n]). Set at submit, used at file:done.
let batchTrackIds: number[] = [];
// Destination bin chosen in the batch folder tree (forward-slash rel; "" = library root). Kept
// across renders so the choice doesn't reset while triaging.
export let batchBin = "";

/** Répartit la sélection courante (queueBatchSel, pistes fileables uniquement) par rail SOURCE,
 *  pour n'afficher que les sélecteurs de format qui s'appliquent et imposer la cible au bon rail.
 *
 *  `unknown` et `null` (piste pas encore analysée) comptent avec le lossy à l'affichage — c'est le
 *  groupe « pas de choix » — mais au moment de filer, aucune cible ne leur est envoyée : le backend
 *  la dérive lui-même du rail réel (`encode::target_for`). */
function batchSelectionByRail(): { lossless: number; lossy: number } {
  let lossless = 0;
  let lossy = 0;
  for (const it of currentItems) {
    if (!queueBatchSel.has(it.id)) continue;
    if (it.verdict === "fake") continue;
    if (it.rail === "lossless") lossless += 1;
    else lossy += 1;
  }
  return { lossless, lossy };
}

/** Les DEUX blocs de format du rail Lot, un par rail source (décision Antoine du 2026-07-28,
 *  PLAN.md § arbitrages point 1). Le rail lossless a un vrai choix — descendre un lossless en MP3
 *  est légitime, ce n'est pas de l'upscale. Le rail lossy n'en a aucun et le dit en toutes lettres
 *  plutôt qu'en options grisées (variante A, choisie sur maquette contre la variante B).
 *
 *  Chaque groupe n'apparaît que si la sélection en contient : sur un lot 100 % MP3, aucun sélecteur
 *  AIFF/WAV ne s'affiche pour rien. */
function formatBlocksHtml(): string {
  const { lossless: nLossless, lossy: nLossy } = batchSelectionByRail();
  const losslessBlock = nLossless
    ? `<div class="sift-rail-fmt-group"><span class="col-h">Lossless · ${nLossless}</span><div class="sift-seg sift-seg-thumbed" id="sift-batch-fmt-seg">` +
      `<div class="sift-seg-thumb"></div>` +
      (["mp3_320", "aiff_16_44", "wav_16_44"] as Target[])
        .map(
          (t) =>
            `<button class="sift-seg-opt${batchLosslessFormat === t ? " on" : ""}" data-sift="batchformat" data-t="${t}">${TARGET_LABEL[t]}</button>`,
        )
        .join("") +
      `</div></div>`
    : "";
  const lossyBlock = nLossy
    ? `<div class="sift-rail-fmt-group"><span class="col-h">Lossy · ${nLossy}</span>` +
      `<span style="font-size:var(--text-md);color:var(--color-text-secondary);white-space:nowrap;padding:var(--space-4) 0">${TARGET_LABEL["mp3_320"]} 320 <span style="color:var(--color-text-tertiary)">— seul format possible</span></span>` +
      `</div>`
    : "";
  return losslessBlock + lossyBlock;
}

// ---------------------------------------------------------------------------
// Global progress zone — feed the "file" row from the per-file filing events.
// ---------------------------------------------------------------------------

let fileClearTimer: ReturnType<typeof setTimeout> | undefined;
let fileStopping = false;
let batchRunning = false;
export function isBatchRunning(): boolean { return batchRunning; }
let lastFileProgress: FileProgress | null = null;

export function pushFileProgress(p: FileProgress) {
  lastFileProgress = p;
  if (p.total <= 0) {
    clearTask("file");
    return;
  }
  if (p.done < p.total) {
    clearTimeout(fileClearTimer);
    setTask("file", { done: p.done, total: p.total, state: "running", stopping: fileStopping });
  } else {
    setTask("file", { done: p.total, total: p.total, state: "done" });
    clearTimeout(fileClearTimer);
    fileClearTimer = setTimeout(() => {
      clearTask("file");
      clearBatchTracklist();
      refreshBatchTracksPreview();
    }, 1200);
  }
  updateBatchTracklist(p.done);
  updateBatchSheet(p.done, p.total);
}

export function onFileStop() {
  if (fileStopping) return;
  fileStopping = true;
  if (lastFileProgress) {
    setTask("file", {
      done: lastFileProgress.done,
      total: lastFileProgress.total,
      state: "running",
      stopping: true,
    });
  }
  fileNote(
    '<i class="ti ti-loader sift-spin" style="font-size:var(--text-md);vertical-align:-1px"></i> Stop requested — finishing the current file…',
  );
  void fileCancel();
}

// ---------------------------------------------------------------------------
// Batch view — zone C = résumé de sélection (wireframe §09)
// ---------------------------------------------------------------------------

/** Renders the batch view: selection summary in #mid, filing rail in #filfoot.
 *  Le board à trois groupes est retiré — la sélection vit dans queueBatchSel
 *  (cases de la colonne de file), pas dans un deuxième tableau. */
export function renderBatch() {
  const mid = requireEl("#mid", "renderBatch");
  const selectedItems = currentItems.filter((it) => queueBatchSel.has(it.id));
  mid.innerHTML = selectionSummaryHtml(selectedItems);
  renderBatchRail();
}

// ---------------------------------------------------------------------------
// Destination
// ---------------------------------------------------------------------------

function batchDest(): string {
  return batchInPlace ? FILE_IN_PLACE : batchBin;
}

function batchDestLabel(): string {
  if (batchInPlace) return IN_PLACE_LABEL;
  if (batchBin.startsWith(EXTERNAL_DEST_PREFIX)) {
    const abs = batchBin.slice(EXTERNAL_DEST_PREFIX.length);
    return abs.split(/[\\/]/).filter(Boolean).pop() || abs;
  }
  return batchBin || "Racine de bibliothèque";
}

/** A folder click in the #fldz tree (batch pick mode) -> set batchBin, drop in-place, re-render. */
export function onBatchBinPick(rel: string): void {
  batchBin = rel;
  batchInPlace = false;
  const fldz = document.getElementById("fldz");
  if (fldz) renderBinsForBatch(fldz, batchBin, onBatchBinPick, batchInPlace);
  renderBatchRail();
}

function ensureBatchDestUI(): void {
  const fldz = document.getElementById("fldz");
  if (!fldz) return;
  setBinPickInert(batchInPlace);
  fldz.style.display = "";
  const treeWrap = fldz.querySelector<HTMLElement>(".sift-fldz-tree");
  if (treeWrap) {
    treeWrap.style.opacity = batchInPlace ? ".4" : "1";
    treeWrap.style.pointerEvents = batchInPlace ? "none" : "auto";
  }
}

// ---------------------------------------------------------------------------
// Format thumb
// ---------------------------------------------------------------------------

function positionBatchFmtThumb(): void {
  const seg = document.getElementById("sift-batch-fmt-seg");
  if (seg) slideSegThumb(seg, "[data-sift='batchformat'].on");
}

// ---------------------------------------------------------------------------
// Right rail — destination + format + progress (actions live in the selection summary)
// ---------------------------------------------------------------------------

function renderBatchRail() {
  const foot = requireEl("#filfoot", "renderBatchRail");
  // Le mode Détail masque ce pied depuis la décision V2b (filing.ts `hidePanelFoot`, 2026-08-30) :
  // le mode Lot, lui, le garde — carte et emplacement bas inchangés. Le démasquer ici est la seule
  // touche de ce chantier sur le Lot, et elle ne change rien à ce qui s'y peint.
  foot.hidden = false;
  requireEl("#fldz", "renderBatchRail");
  ensureBatchDestUI();
  const keepTracks = batchRunning ? foot.querySelector("#sift-batch-tracks") : null;
  const keepNote = foot.querySelector("[data-file-note]");
  if (foot.querySelector("#sift-progress-zone")) homeProgressZone();

  const destBlock = `<button data-fil="destbtn" class="sift-dest-btn"><span class="sift-dest-btn-label">Destination</span><span class="sift-fil-bin">${esc(
    batchDestLabel(),
  )}</span><i class="ti ti-chevron-down sift-dest-btn-caret"></i></button>`;
  const formatBlock = `<div id="sift-batch-fmt" style="display:contents">${formatBlocksHtml()}</div>`;
  const stopSlot = batchRunning
    ? `<div class="sift-baction-slot"><button data-sift="batchstop" class="sift-baction sift-baction--quiet">Stop</button></div>`
    : "";

  foot.innerHTML =
    destBlock +
    formatBlock +
    `<div class="sift-rail-spacer"></div>` +
    stopSlot +
    `<div id="sift-batch-progress" style="flex-basis:100%"></div>` +
    `<div id="sift-batch-tracks" style="flex-basis:100%"></div>`;
  if (keepNote) foot.insertAdjacentElement("afterbegin", keepNote);
  if (keepTracks) foot.querySelector("#sift-batch-tracks")!.replaceWith(keepTracks);
  else refreshBatchTracksPreview();
  foot.querySelector('[data-fil="destbtn"]')?.addEventListener("click", (e) => {
    e.stopPropagation();
    toggleDestPopover();
  });
  ensureDestPopoverAutoClose();
  mountProgressZone(requireEl("#sift-batch-progress", "renderBatchRail progress slot"));
  repositionDestPopoverIfOpen();
  positionBatchFmtThumb();
}

// ---------------------------------------------------------------------------
// Filing launch
// ---------------------------------------------------------------------------

async function runBatchFile(ids: number[]) {
  if (ids.length === 0) return;
  fileStopping = false;
  lastFileProgress = null;
  batchRunning = true;
  const mid = document.getElementById("mid");
  if (mid) mid.innerHTML = "";
  renderBatchRail();
  batchTrackIds = ids;
  startBatchTracklist(ensureBatchTracklistHost(), ids.map(batchTrackItem));
  setTask("file", { done: 0, total: ids.length, state: "running" });
  const inspector = document.querySelector<HTMLElement>(".sift-inspector");
  if (inspector) showBatchSheet(inspector, ids, batchTrackName);
  // Cible imposée UNIQUEMENT aux pistes de rail lossless. Les autres — lossy, `unknown`, pas
  // encore analysées — restent absentes de la table : le backend dérive la cible depuis le rail
  // source (`encode::target_for`), ce qui rend l'upscale impossible par construction.
  const railById = new Map(currentItems.map((it) => [it.id, it.rail]));
  const targets: Record<number, Target> = {};
  for (const id of ids) {
    if (railById.get(id) === "lossless") targets[id] = batchLosslessFormat;
  }
  try {
    await fileBatch(ids, batchDest(), targets);
  } catch (err) {
    const code = String(err);
    // Le Lot rend son refus dans `fileNote` — une ligne de texte SOUS le rail, pas un toast : il n'a
    // donc pas de bouton d'action où loger « Choisir la racine ». Plutôt qu'inventer un contrôle
    // dans cette ligne, on double la note par le MÊME toast actionnable que le Détail (issue #54) :
    // un seul mot d'UI pour un seul refus, et l'action existe des deux côtés.
    if (code.includes("NoLibraryRoot")) {
      fileNote(
        "Conversion bloquée — aucune racine de bibliothèque.",
        "var(--color-text-danger)",
      );
      toast(
        "Conversion bloquée — aucune racine de bibliothèque.",
        true,
        openSettingsScreen,
        "Choisir la racine",
      );
    } else {
      fileNote("Échec du lancement de la conversion — réessaie", "var(--color-text-danger)");
    }
    console.error("file_batch launch failed", err);
  }
}

function batchTrackName(id: number): string {
  const it = currentItems.find((q) => q.id === id);
  if (!it) return `#${id}`;
  return [it.artist, it.title].filter(Boolean).join(" — ") || it.filename || it.path;
}

function batchTrackItem(id: number): { id: number; name: string } {
  return { id, name: batchTrackName(id) };
}

function refreshBatchTracksPreview(): void {
  if (reviewMode !== "batch" || batchRunning) return;
  const host = document.getElementById("sift-batch-tracks");
  if (!host) return;
  host.innerHTML = "";
}

function ensureBatchTracklistHost(): HTMLElement {
  let el = document.getElementById("sift-batch-tracks");
  if (!el) {
    el = document.createElement("div");
    el.id = "sift-batch-tracks";
    document.getElementById("filfoot")?.appendChild(el);
  }
  return el;
}

function fileNote(html: string, color = "var(--color-text-secondary)") {
  const foot = document.getElementById("filfoot");
  if (!foot) return;
  foot.querySelector("[data-file-note]")?.remove();
  foot.insertAdjacentHTML(
    "afterbegin",
    `<div data-file-note style="font-size:var(--text-sm);color:${color};margin-bottom:10px">${html}</div>`,
  );
}

let refreshHook: (() => Promise<void>) | null = null;

export function registerRefreshHook(fn: () => Promise<void>): void {
  refreshHook = fn;
}

export async function onFileBatchDone(res: BatchResult) {
  fileStopping = false;
  batchRunning = false;
  if (reviewMode === "batch") {
    const mid = document.getElementById("mid");
    if (mid) mid.innerHTML = selectionSummaryHtml(currentItems.filter((it) => queueBatchSel.has(it.id)));
  }
  const processed = res.cancelled ? batchTrackIds.slice(0, lastFileProgress?.done ?? 0) : batchTrackIds;
  const failed = new Set(res.needs_validation);
  finishBatchTracklist(processed.filter((id) => !failed.has(id)), res.needs_validation);
  if (res.cancelled) {
    clearTimeout(fileClearTimer);
    const lp = lastFileProgress;
    if (lp) {
      setTask("file", { done: lp.done, total: lp.total, state: "done" });
      fileClearTimer = setTimeout(() => {
        clearTask("file");
        clearBatchTracklist();
        refreshBatchTracksPreview();
      }, 1200);
    } else {
      clearTask("file");
      clearBatchTracklist();
      refreshBatchTracksPreview();
    }
  }
  const nKo = res.needs_validation.length;
  const plural = (n: number, s: string, p: string) => (n > 1 ? p : s);
  const base = nKo
    ? `${res.filed} ${plural(res.filed, "rangée", "rangées")} · ${nKo} ${plural(nKo, "à vérifier", "à vérifier")}`
    : `${res.filed} ${plural(res.filed, "piste rangée", "pistes rangées")}`;
  const tone =
    res.filed === 0 && nKo
      ? { icon: "ti-alert-triangle", color: "var(--color-text-danger)" }
      : nKo
        ? { icon: "ti-alert-triangle", color: "var(--color-text-warning)" }
        : { icon: "ti-check", color: "var(--color-text-success)" };
  await refreshHook?.();
  fileNote(
    `<i class="ti ${tone.icon}" style="font-size:var(--text-md);vertical-align:-1px"></i> ${
      res.cancelled ? `Conversion interrompue · ${base}` : base
    }`,
    tone.color,
  );
  transformToReport(res);
}

async function runBatchDiscard(ids: number[]) {
  if (ids.length === 0) return;
  batchRunning = true;
  renderBatchRail();
  try {
    await rejectBatch(ids);
  } catch (err) {
    console.error("reject_batch failed", err);
    fileNote("Échec de l'écartement — réessaie", "var(--color-text-danger)");
  } finally {
    batchRunning = false;
    await refreshHook?.();
  }
}

// ---------------------------------------------------------------------------
// Click handlers
// ---------------------------------------------------------------------------

/** Routes the batch mode's delegated clicks — format, open-in-detail, stop.
 *  Les handlers du board (batchpick, batchpickfake, batchgroup, batchcollapse, batchmore) et
 *  l'armement (batchaction, batchcancelconfirm) ont été retirés avec le board — la sélection
 *  vit dans queueBatchSel (queue-panel.ts), les actions dans le résumé de sélection (zone C). */
export function handleBatchAction(el: HTMLElement, act: string, e: MouseEvent): boolean {
  if (act === "batchformat") {
    e.stopPropagation();
    batchLosslessFormat = el.dataset.t as Target;
    document
      .querySelectorAll<HTMLElement>("#sift-batch-fmt-seg [data-sift='batchformat']")
      .forEach((b) => b.classList.toggle("on", b.dataset.t === batchLosslessFormat));
    // Le pouce GLISSE sur le nœud existant, et rien d'autre ne bouge : jusqu'au 2026-09-10 un
    // `requestAnimationFrame(renderBatchRail)` suivait, qui reconstruisait tout le rail par
    // `innerHTML` à la frame suivante — le pouce neuf repartait de `left:0` (la position MP3) et
    // reglissait jusqu'à la cible (mesuré : 52 px à 70 ms au lieu d'un point entre 45 et 89).
    // C'est le piège de CLAUDE.md § Front : une transition n'anime rien sur un nœud recréé. Rien
    // dans le rail ne lit `batchLosslessFormat` — seule la confirmation de rangement le lit, au
    // moment de ranger (`handleBatchQueueAction`).
    positionBatchFmtThumb();
  } else if (act === "batchopen") {
    e.stopPropagation();
    const id = Number(el.dataset.id);
    const item = currentItems.find((it) => it.id === id);
    enterDetailMode();
    const mid = requireEl("#mid", "batchopen");
    if (item && mid) {
      void openFilingInto(mid, item);
      prefetchNextAfter(item.id);
    }
  } else if (act === "batchstop") {
    e.stopPropagation();
    onFileStop();
  } else if (act === "batchsheetdetail") {
    e.stopPropagation();
    const id = Number(el.dataset.id);
    const item = currentItems.find((it) => it.id === id);
    closeBatchSheet();
    enterDetailMode();
    const mid = requireEl("#mid", "batchsheetdetail");
    if (item && mid) {
      void openFilingInto(mid, item);
      prefetchNextAfter(item.id);
    }
  } else if (act === "batchsheetclose") {
    e.stopPropagation();
    closeBatchSheet();
  } else {
    return false;
  }
  return true;
}

/** File ou écarte la sélection de la colonne de file (queueBatchSel).
 *  Déclenché par les boutons du résumé de sélection (zone C) ou le menu contextuel.
 *  Au-delà de BATCH_CONFIRM_THRESHOLD, demande confirmation via la modale in-app
 *  (window.confirm() est inutilisable — un clic synthétique l'a traversée, incident 2026-07-03).
 *  Format lossless : la cible du segmenté du rail est appliquée ; le reste dérive du rail source
 *  côté backend (`encode::target_for`), ce qui empêche l'upscale par construction. */
export async function handleBatchQueueAction(action: "file" | "discard"): Promise<void> {
  const selected = currentItems.filter((it) => queueBatchSel.has(it.id));
  if (selected.length === 0) return;

  if (action === "file") {
    const fileIds = selected.filter((it) => it.verdict !== "fake").map((it) => it.id);
    if (fileIds.length === 0) return;
    if (fileIds.length > BATCH_CONFIRM_THRESHOLD && !skipBatchConfirm) {
      const fakeN = selected.filter((it) => it.verdict === "fake").length;
      const { lossless, lossy } = batchSelectionByRail();
      const fmtParts: string[] = [];
      if (lossless > 0) fmtParts.push(TARGET_LABEL[batchLosslessFormat]);
      if (lossy > 0) fmtParts.push("MP3 320");
      const alertData: BatchAlertData = {
        fileCount: fileIds.length,
        fakeCount: fakeN,
        destLabel: batchDestLabel(),
        formatSummary: fmtParts.join(" + "),
      };
      const result = await confirmBatchAlert(alertData);
      if (!result.confirmed) return;
      if (result.skipFuture) skipBatchConfirm = true;
    }
    void runBatchFile(fileIds);
  } else {
    const fakeIds = selected.filter((it) => it.verdict === "fake").map((it) => it.id);
    void runBatchDiscard(fakeIds);
  }
}

/** Handles the "file in place" checkbox change. */
export function onBatchInPlaceChange(checked: boolean): void {
  batchInPlace = checked;
  const fldz = document.getElementById("fldz");
  if (fldz) renderBinsForBatch(fldz, batchBin, onBatchBinPick, batchInPlace);
  renderBatchRail();
}
