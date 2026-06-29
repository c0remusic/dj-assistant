// Journal tab + extended journal page.
//
// renderJournal()         — current-session view (~1 Hz max, user clicks a tab).
// renderJournalExtended() — full-history view, opened by "Voir tout" toggle.
//
// DOM mutation strategy: each render sets innerHTML once on #content, then installs ONE
// delegated click listener on #content. Old listeners are garbage-collected with the old
// nodes — no accumulation across re-renders. Never addEventListener per row.

import type { JournalEntry } from "../shared/contracts";
import { listJournal, getSessionId, revertBatch } from "./ipc";
import { requireEl } from "./dom";

// ---------------------------------------------------------------------------
// Pure helpers
// ---------------------------------------------------------------------------

function basenameNoExt(p: string | null): string {
  if (!p) return "";
  const i = Math.max(p.lastIndexOf("/"), p.lastIndexOf("\\"));
  const seg = i >= 0 ? p.slice(i + 1) : p;
  const dot = seg.lastIndexOf(".");
  return dot > 0 ? seg.slice(0, dot) : seg;
}

/** Last 2 path segments — e.g. "House/Larry Heard — Mystery.aiff". Never an absolute path. */
function rel2(p: string | null): string {
  if (!p) return "";
  const parts = p.replace(/\\/g, "/").split("/").filter(Boolean);
  return parts.length >= 2 ? parts.slice(-2).join("/") : parts.join("/");
}

// ---------------------------------------------------------------------------
// Categories
// ---------------------------------------------------------------------------

interface Cat {
  id: "filed" | "trash" | "reject";
  label: string;
  massLabel: string;
  massColor: string;
  entries: JournalEntry[];
}

function filterByCat(entries: JournalEntry[], catId: string): JournalEntry[] {
  if (catId === "filed") return entries.filter(e => e.kind === "convert" || e.kind === "move");
  if (catId === "trash") return entries.filter(e => e.kind === "trash");
  if (catId === "reject") return entries.filter(e => e.kind === "reject");
  return [];
}

function buildCategories(entries: JournalEntry[]): Cat[] {
  const filed = filterByCat(entries, "filed");
  const trash = filterByCat(entries, "trash");
  const reject = filterByCat(entries, "reject");
  const n = filed.length;
  return [
    {
      id: "filed",
      label: "FILÉS",
      massLabel: `↩ Défiler les ${n} affichés`,
      massColor: "var(--color-text-danger)",
      entries: filed,
    },
    {
      id: "trash",
      label: "JETÉS",
      massLabel: "Restaurer",
      massColor: "var(--color-text-warning)",
      entries: trash,
    },
    {
      id: "reject",
      label: "REJETÉS",
      massLabel: "Remettre en file",
      massColor: "var(--color-text-secondary)",
      entries: reject,
    },
  ];
}

// ---------------------------------------------------------------------------
// HTML builders
// ---------------------------------------------------------------------------

function rowHtml(e: JournalEntry): string {
  const name = basenameNoExt(e.from_path ?? e.to_path);
  const dest = rel2(e.to_path);
  const bid = e.batch_id;
  return `<div class="jrnl-row" data-batch-id="${bid}">\
<div class="jrnl-row-main">\
<span class="jrnl-name">${name}</span>\
<span class="jrnl-dest">${dest ? "→ " + dest : ""}</span>\
</div>\
<button class="jrnl-revert" data-jact="revert" data-batch-id="${bid}" title="Annuler">&#x21A9;</button>\
</div>`;
}

function sectionHtml(cat: Cat, filedEntries: JournalEntry[]): string {
  if (cat.entries.length === 0) return "";
  const rows = cat.entries.map(rowHtml).join("");

  let footer = "";
  if (cat.id === "filed" && filedEntries.length > 0) {
    const last = filedEntries[0];
    const n = last.track_count;
    footer = `<div class="jrnl-cat-foot">\
<button class="jrnl-last-batch" data-jact="last-batch" data-batch-id="${last.batch_id}" data-track-count="${n}">\
↩ Annuler le dernier batch (${n} morceau${n > 1 ? "x" : ""})\
</button>\
</div>`;
  }

  return `<details class="jrnl-cat" open data-cat="${cat.id}">\
<summary class="jrnl-cat-hd">\
<i class="ti ti-chevron-right jrnl-cat-chev" aria-hidden="true"></i>\
<span class="col-h jrnl-cat-label">${cat.label}</span>\
<span class="jrnl-cat-badge">${cat.entries.length}</span>\
<button class="jrnl-mass" data-jact="mass-revert" data-cat="${cat.id}" style="color:${cat.massColor}">${cat.massLabel}</button>\
</summary>\
<div class="jrnl-rows">${rows}</div>\
${footer}\
</details>`;
}

function headerHtml(activeMode: "session" | "all"): string {
  const sessionCls = activeMode === "session" ? " on" : "";
  const allCls = activeMode === "all" ? " on" : "";
  return `<div class="jrnl-hd">\
<span>Journal</span>\
<div class="jrnl-mode">\
<button class="jrnl-mode-btn${sessionCls}" data-jact="mode-session">Session courante</button>\
<button class="jrnl-mode-btn${allCls}" data-jact="mode-all">Tout l'historique</button>\
</div>\
</div>`;
}

// ---------------------------------------------------------------------------
// Delegated click handler — installed once per render on #content
// (frequency: ~never during normal use; only on user click events)
// ---------------------------------------------------------------------------

// AbortController prevents listener accumulation: #content is permanent and never re-created,
// so without this each renderJournal() call would stack another "click" listener on it.
let _delegateAbort: AbortController | null = null;

function humanError(err: unknown): string {
  const raw = err instanceof Error ? err.message : String(err);
  console.error("[journal] revert_batch failed:", raw);
  if (raw.includes("destination occupied"))
    return "Fichier déjà à l'emplacement d'origine — doublon probable (sync cloud ?).";
  if (raw.includes("source gone"))
    return "Fichier introuvable à destination — déplacé ou supprimé manuellement ?";
  if (raw.includes("newer action"))
    return "Action plus récente à annuler d'abord.";
  return `Revert échoué : ${raw}`;
}

/** Inject a persistent status banner at the top of .jrnl-wrap (no animation — stays until next render). */
function injectBanner(root: HTMLElement, text: string, kind: "ok" | "warn"): void {
  const wrap = root.querySelector<HTMLElement>(".jrnl-wrap");
  if (!wrap) return;
  wrap.querySelector(".jrnl-banner")?.remove();
  const el = document.createElement("div");
  el.className = `jrnl-banner jrnl-banner--${kind}`;
  el.textContent = text;
  wrap.prepend(el);
}

function installDelegate(root: HTMLElement, allEntries: JournalEntry[]): void {
  // Abort previous delegated listener before adding a new one.
  _delegateAbort?.abort();
  _delegateAbort = new AbortController();

  // .jrnl-mass is inside <summary> — its click toggles <details> unless stopped.
  // Direct listener: stopPropagation() keeps the section open, then handles mass revert.
  // Dies on next render with the old nodes — no accumulation.
  root.querySelectorAll<HTMLButtonElement>(".jrnl-mass").forEach(btn => {
    btn.addEventListener("click", ev => {
      ev.stopPropagation();
      const catId = btn.dataset.cat!;
      const catEntries = filterByCat(allEntries, catId);
      const totalTracks = catEntries.reduce((s, e) => s + e.track_count, 0);
      const label =
        catId === "filed" ? "Défiler" : catId === "trash" ? "Restaurer" : "Remettre en file";
      if (!window.confirm(`${label} les ${totalTracks} morceaux affichés ?`)) return;
      btn.disabled = true;
      // Sequential — rusqlite Mutex is non-reentrant; concurrent IPC calls would deadlock.
      // Per-entry DOM update so the user sees progress as each track is processed.
      void (async () => {
        let failCount = 0;
        for (const e of catEntries) {
          const rowEl = root.querySelector<HTMLElement>(`.jrnl-row[data-batch-id="${e.batch_id}"]`);
          const rowBtn = rowEl?.querySelector<HTMLButtonElement>("[data-jact='revert']");
          if (rowBtn) { rowBtn.textContent = "⏳"; rowBtn.disabled = true; }
          rowEl?.classList.add("jrnl-row--loading");
          try {
            await revertBatch(e.batch_id);
            if (rowBtn) rowBtn.textContent = "✓";
            rowEl?.classList.replace("jrnl-row--loading", "jrnl-row--reverted");
          } catch (err) {
            console.error("[journal] revert_batch failed:", err);
            failCount++;
            if (rowBtn) { rowBtn.textContent = "↩"; rowBtn.disabled = false; }
            rowEl?.classList.remove("jrnl-row--loading");
          }
        }
        const ok = catEntries.length - failCount;
        if (ok > 0 && failCount === 0) {
          injectBanner(root, `↩ ${ok} action${ok > 1 ? "s" : ""} annulée${ok > 1 ? "s" : ""}`, "ok");
        } else if (ok > 0) {
          injectBanner(root, `↩ ${ok} ok — ${failCount} introuvable${failCount > 1 ? "s" : ""}`, "warn");
        } else {
          injectBanner(root, `${failCount} morceau${failCount > 1 ? "x" : ""} introuvable${failCount > 1 ? "s" : ""} — déjà revertés ?`, "warn");
        }
      })();
    });
  });

  // Per-row revert: DIRECT listener on each button, not delegated.
  // Bypasses any potential bubbling/closest() issue. Buttons are recreated on each render
  // (via innerHTML), so old listeners are GC'd — no accumulation.
  root.querySelectorAll<HTMLButtonElement>("[data-jact='revert']").forEach(btn => {
    btn.addEventListener("click", () => {
      const bid = btn.dataset.batchId;
      if (!bid) { console.error("[journal] missing data-batch-id on revert button"); return; }
      const row = btn.closest<HTMLElement>(".jrnl-row");
      btn.textContent = "⏳";
      btn.disabled = true;
      row?.classList.add("jrnl-row--loading");
      revertBatch(bid)
        .then(() => {
          btn.textContent = "✓";
          row?.classList.replace("jrnl-row--loading", "jrnl-row--reverted");
          injectBanner(root, "↩ Remis en file", "ok");
        })
        .catch(err => {
          btn.textContent = "↩";
          btn.disabled = false;
          row?.classList.remove("jrnl-row--loading");
          injectBanner(root, humanError(err), "warn");
        });
    });
  });

  // Delegated listener for last-batch and mode switches only.
  // (.jrnl-mass and [data-jact='revert'] use direct listeners above — stopPropagation
  // on .jrnl-mass would block them from reaching here anyway.)
  root.addEventListener("click", (ev: MouseEvent) => {
    const t = ev.target as Element;

    // Last-batch revert (confirm only if > 10 tracks)
    const lbBtn = t.closest<HTMLButtonElement>("[data-jact='last-batch']");
    if (lbBtn) {
      const bid = lbBtn.dataset.batchId;
      if (!bid) { console.error("[journal] missing data-batch-id on last-batch"); return; }
      const n = Number(lbBtn.dataset.trackCount ?? 0);
      if (n > 10 && !window.confirm(`Annuler le batch de ${n} morceaux ?`)) return;
      lbBtn.disabled = true;
      revertBatch(bid)
        .then(() => {
          root.querySelector<HTMLElement>(`.jrnl-row[data-batch-id="${bid}"]`)
            ?.classList.add("jrnl-row--reverted");
          injectBanner(root, `↩ Batch de ${n} morceau${n > 1 ? "x" : ""} annulé`, "ok");
        })
        .catch(err => { lbBtn.disabled = false; injectBanner(root, humanError(err), "warn"); });
      return;
    }

    // Mode switches
    if (t.closest("[data-jact='mode-session']")) { void renderJournal(); return; }
    if (t.closest("[data-jact='mode-all']")) { void renderJournalExtended(); return; }
  }, { signal: _delegateAbort.signal });
}

// ---------------------------------------------------------------------------
// Current-session journal
// ---------------------------------------------------------------------------

export async function renderJournal(toast?: string, warn?: string): Promise<void> {
  // Fail-fast: both calls throw on IPC error — no silent fallback.
  const sessionId = await getSessionId();
  const entries = await listJournal(50, sessionId);

  const content = requireEl<HTMLElement>("#content", "renderJournal");

  const cats = buildCategories(entries);
  const filedEntries = cats.find(c => c.id === "filed")!.entries;
  const hasAny = cats.some(c => c.entries.length > 0);

  const sectionsHtml = cats.map(c => sectionHtml(c, filedEntries)).join("");
  const emptyHtml = hasAny ? "" : `<div class="jrnl-empty">Aucune action dans cette session.</div>`;
  const voirToutHtml = hasAny
    ? `<button class="jrnl-voir-tout" data-jact="mode-all">Voir tout l'historique →</button>`
    : "";
  const toastHtml = toast ? `<div class="jrnl-toast" aria-live="polite">${toast}</div>` : "";
  const warnHtml = warn ? `<div class="jrnl-toast jrnl-toast--warn" aria-live="assertive">${warn}</div>` : "";

  content.innerHTML = `<div class="jrnl-wrap">\
${toastHtml}${warnHtml}\
${headerHtml("session")}\
${emptyHtml}\
${sectionsHtml}\
${voirToutHtml}\
</div>`;

  installDelegate(content, entries);
}

// ---------------------------------------------------------------------------
// Extended journal (all sessions)
// ---------------------------------------------------------------------------

function sessionGroupHtml(sessionId: string | null, entries: JournalEntry[]): string {
  const label = sessionId ?? "Antérieur";
  const cats = buildCategories(entries);
  const filedEntries = cats.find(c => c.id === "filed")!.entries;
  const sectionsHtml = cats.map(c => sectionHtml(c, filedEntries)).join("");
  return `<div class="jrnl-session-group">\
<div class="jrnl-session-hd">${label}</div>\
${sectionsHtml}\
</div>`;
}

export async function renderJournalExtended(): Promise<void> {
  // No session filter — all sessions, newest first (backend returns DESC).
  const all = await listJournal(500);

  const content = requireEl<HTMLElement>("#content", "renderJournalExtended");

  // Group by session_id preserving insertion order (already newest-first from the backend).
  const sessionOrder: (string | null)[] = [];
  const bySession = new Map<string | null, JournalEntry[]>();
  for (const e of all) {
    const key = e.session_id;
    if (!bySession.has(key)) {
      sessionOrder.push(key);
      bySession.set(key, []);
    }
    bySession.get(key)!.push(e);
  }

  const groupsHtml = sessionOrder
    .map(sid => sessionGroupHtml(sid, bySession.get(sid)!))
    .join("");

  const bodyHtml =
    all.length === 0
      ? `<div class="jrnl-empty">Aucune action enregistrée.</div>`
      : groupsHtml;

  content.innerHTML = `<div class="jrnl-wrap">\
${headerHtml("all")}\
${bodyHtml}\
</div>`;

  installDelegate(content, all);
}
