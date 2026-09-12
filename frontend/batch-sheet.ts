// Batch filing sheet — non-modal progress + terminal report (wireframe § 17).
// Inserted inside .sift-inspector, slides from top. The queue column stays interactive.
import type { BatchResult } from "../shared/contracts";
import { esc } from "./dom";

let sheetEl: HTMLElement | null = null;
let orderedIds: number[] = [];
let nameFn: (id: number) => string = (id) => `#${id}`;
let trackRows: Map<number, HTMLElement> = new Map();

export function showBatchSheet(
  parent: HTMLElement,
  ids: number[],
  nameResolver: (id: number) => string,
): void {
  closeBatchSheet();
  orderedIds = ids;
  nameFn = nameResolver;
  trackRows = new Map();

  const sheet = document.createElement("div");
  sheet.className = "sift-batch-sheet";
  sheet.dataset.state = "progress";
  const total = ids.length;
  sheet.innerHTML =
    `<div class="sift-bs-head">` +
    `<div class="sift-bs-title">Rangement de ${total} piste${total > 1 ? "s" : ""}</div>` +
    `<button class="sift-baction sift-baction--quiet sift-bs-stop" data-sift="batchstop">Arrêter</button>` +
    `</div>` +
    `<progress class="sift-bs-bar" value="0" max="${total}"></progress>` +
    `<div class="sift-bs-step"></div>` +
    `<details class="sift-bs-details">` +
    `<summary>Afficher les détails</summary>` +
    `<div class="sift-bs-tracks"></div>` +
    `</details>`;

  const tracksHost = sheet.querySelector(".sift-bs-tracks")!;
  for (const id of ids) {
    const row = document.createElement("div");
    row.className = "sift-bs-track";
    row.dataset.state = "wait";
    row.textContent = nameResolver(id);
    tracksHost.appendChild(row);
    trackRows.set(id, row);
  }

  parent.insertBefore(sheet, parent.firstChild);
  sheetEl = sheet;
  requestAnimationFrame(() => sheet.classList.add("sift-batch-sheet--open"));
}

export function updateBatchSheet(done: number, total: number): void {
  if (!sheetEl || sheetEl.dataset.state !== "progress") return;

  const bar = sheetEl.querySelector<HTMLProgressElement>(".sift-bs-bar");
  if (bar) {
    bar.value = done;
    bar.max = total;
  }

  const step = sheetEl.querySelector(".sift-bs-step");
  if (step) {
    const currentId = done < orderedIds.length ? orderedIds[done] : undefined;
    const name = currentId != null ? nameFn(currentId) : "";
    step.textContent =
      done < total
        ? `Conversion ${done + 1} sur ${total}` + (name ? ` · ${name}` : "")
        : `${total} sur ${total}`;
  }

  for (let i = 0; i < orderedIds.length; i++) {
    const row = trackRows.get(orderedIds[i]);
    if (!row) continue;
    if (i < done) row.dataset.state = "done";
    else if (i === done) row.dataset.state = "run";
  }
}

export function transformToReport(res: BatchResult): void {
  if (!sheetEl) return;
  sheetEl.dataset.state = "report";

  const errorIds = new Set(res.errors.map((e) => e.track_id));
  const validationOnly = res.needs_validation.filter((id) => !errorIds.has(id));
  const nTotal = res.filed + res.needs_validation.length;
  const nKo = res.needs_validation.length;

  const parts: string[] = [];
  parts.push(`${nTotal} traitée${nTotal > 1 ? "s" : ""}`);
  if (res.filed > 0) parts.push(`${res.filed} rangée${res.filed > 1 ? "s" : ""}`);
  if (nKo > 0) parts.push(`${nKo} à vérifier`);

  let html = `<div class="sift-bs-title sift-bs-title--report">${parts.join(" · ")}</div>`;

  if (res.filed_ids.length > 0) {
    html += sectionHtml("Rangées", res.filed_ids, "ok");
  }
  if (validationOnly.length > 0) {
    html += sectionHtml("À valider", validationOnly, "warning");
  }
  if (res.errors.length > 0) {
    html += sectionHtml(
      "Échecs",
      res.errors.map((e) => e.track_id),
      "error",
      new Map(res.errors.map((e) => [e.track_id, e.message])),
    );
  }

  html +=
    `<div class="sift-bs-footer">` +
    `<button class="sift-baction sift-baction--quiet" data-sift="batchsheetclose">Fermer</button>` +
    `</div>`;

  sheetEl.innerHTML = html;
}

function sectionHtml(
  label: string,
  ids: number[],
  tone: "ok" | "warning" | "error",
  errorMsgs?: Map<number, string>,
): string {
  const items = ids
    .map((id) => {
      const name = esc(nameFn(id));
      const err = errorMsgs?.get(id);
      return (
        `<div class="sift-bs-item">` +
        `<span class="sift-bs-item-name">${name}</span>` +
        (err ? `<span class="sift-bs-item-err">${esc(err)}</span>` : "") +
        `<button class="sift-bs-item-link" data-sift="batchsheetdetail" data-id="${id}">Détail</button>` +
        `</div>`
      );
    })
    .join("");

  return (
    `<details class="sift-bs-section sift-bs-section--${tone}" open>` +
    `<summary>${esc(label)} (${ids.length})</summary>` +
    `<div class="sift-bs-items">${items}</div>` +
    `</details>`
  );
}

export function closeBatchSheet(): void {
  sheetEl?.remove();
  sheetEl = null;
  trackRows.clear();
}

export function isBatchSheetOpen(): boolean {
  return sheetEl !== null;
}
