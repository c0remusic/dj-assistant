// Résumé de sélection pour la zone C en mode Lot.
// Module pur — aucun accès DOM. Importé par batch-panel.ts.
import type { QueueItem } from "../shared/contracts";
import { esc } from "./dom";

/** Construit le HTML du panneau de résumé de sélection.
 *  Rendu EN TÊTE du batch board quand la sélection est non vide.
 *  Les boutons portent data-sift="batchqueuefile" / "batchqueuediscard"
 *  pour le routing délégué de sift-live.ts.
 *  Classes de bouton : .sift-baction .sift-baction--primary / --quiet (grammaire batch existante). */
export function selectionSummaryHtml(selected: QueueItem[]): string {
  const n = selected.length;
  if (n === 0) {
    return `<div class="sift-bsel-empty">Sélectionne des pistes dans la file</div>`;
  }
  const ok    = selected.filter((it) => it.verdict === "ok").length;
  const fake  = selected.filter((it) => it.verdict === "fake").length;
  const grey  = selected.filter((it) => it.verdict === "grey").length;
  const other = n - ok - fake - grey;

  // Durée totale (si champs disponibles grâce au contrat S1)
  const withDur = selected.filter((it) => it.duration != null);
  const totalSec = withDur.reduce((s, it) => s + (it.duration ?? 0), 0);
  const durStr =
    totalSec > 0
      ? (() => {
          const h = Math.floor(totalSec / 3600);
          const m = Math.floor((totalSec % 3600) / 60);
          const s = Math.floor(totalSec % 60);
          return h > 0
            ? `${h}h ${String(m).padStart(2, "0")}m`
            : `${m}m ${String(s).padStart(2, "0")}s`;
        })()
      : null;

  // Format dominant (si disponible)
  const fmts: Record<string, number> = {};
  for (const it of selected) {
    if (it.declared_fmt) fmts[it.declared_fmt] = (fmts[it.declared_fmt] ?? 0) + 1;
  }
  const fmtStr =
    Object.entries(fmts)
      .sort((a, b) => b[1] - a[1])
      .map(([f, c]) => `${c > 1 ? c + " " : ""}${esc(f.toUpperCase())}`)
      .join(" · ") || null;

  const verdictPills = [
    ok    > 0 ? `<span class="sift-bsel-pill ok">${ok} ok</span>` : "",
    fake  > 0 ? `<span class="sift-bsel-pill fake">${fake} faux</span>` : "",
    grey  > 0 ? `<span class="sift-bsel-pill grey">${grey} à vérifier</span>` : "",
    other > 0 ? `<span class="sift-bsel-pill other">${other} en cours</span>` : "",
  ]
    .filter(Boolean)
    .join("");

  const fileN    = ok + grey; // fileables (ok + grey, backend décide)
  const discardN = fake;

  return (
    `<div class="sift-bsel">` +
    `<div class="sift-bsel-count">${n}<span class="sift-bsel-count-label">piste${n > 1 ? "s" : ""} sélectionnée${n > 1 ? "s" : ""}</span></div>` +
    (verdictPills ? `<div class="sift-bsel-pills">${verdictPills}</div>` : "") +
    (durStr || fmtStr
      ? `<div class="sift-bsel-meta">${[durStr, fmtStr].filter(Boolean).join(" · ")}</div>`
      : "") +
    `<div class="sift-bsel-actions">` +
    (fileN > 0
      ? `<button class="sift-baction sift-baction--primary" data-sift="batchqueuefile">Ranger ${fileN} piste${fileN > 1 ? "s" : ""}</button>`
      : `<button class="sift-baction sift-baction--primary" disabled>Ranger</button>`) +
    (discardN > 0
      ? `<button class="sift-baction sift-baction--quiet" data-sift="batchqueuediscard">Écarter ${discardN} faux</button>`
      : `<button class="sift-baction sift-baction--quiet" disabled>Écarter</button>`) +
    `</div>` +
    `</div>`
  );
}
