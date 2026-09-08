// Rekordbox integration screen — extracted from sift-live.ts (clean-architecture audit F1,
// 2026-07-09). Dispatch for mdb*/mds*/mas*/rkbreexport actions now lives here in
// handleRekordboxAction (unlike ecartes-view.ts, where dispatch stays centralized in
// sift-live.ts's delegated #pa handler) — sift-live.ts's #pa handler just calls it and returns
// early if it handled the action. It mutates the Set/Map state below via method calls
// (add/delete/set), which works fine across the module boundary; only bare reassigned
// primitives would need boxing, and none of this module's exported state is bare-reassigned
// from outside it.
import {
  rekordboxStatus,
  rekordboxMasterdbPendingRepairs,
  rekordboxMasterdbScanPlaylistDuplicates,
  rekordboxMasterdbPendingMetadataSyncs,
  rekordboxMasterdbPendingArtworkSyncs,
  rekordboxMasterdbDismissRepair,
  rekordboxMasterdbResolveAmbiguous,
  rekordboxMasterdbApplyRepairs,
  rekordboxMasterdbDedupPlaylistGroup,
  rekordboxMasterdbDismissMetadataSync,
  rekordboxMasterdbResolveAmbiguousMetadataSync,
  rekordboxMasterdbApplyMetadataSyncs,
  rekordboxMasterdbDismissArtworkSync,
  rekordboxMasterdbResolveAmbiguousArtworkSync,
  rekordboxMasterdbApplyArtworkSyncs,
} from "./ipc";
import type {
  RekordboxLinkStatus,
  PendingMasterdbRepair,
  CandidateTrack,
  PlaylistDuplicateGroupDto,
  PendingMetadataSync,
  PendingArtworkSync,
  ApplyMetadataSyncOutcome,
} from "../shared/contracts";
import { requireEl, esc } from "./dom";
import { isStaleViewRender, viewEpoch } from "./view-epoch";
import { toast } from "./filing-toast";
import { emptyStateHtml, wireEmptyState } from "./empty-state";
import { confirmAction } from "./confirm-modal";
import { mountBarActions } from "./toolbar";
import { sessionLabel } from "./session-label";

// M8 Tier 1 repairs section state — module-level, NOT reset on every render. Filtered against
// the live pending/ambiguous rows each render so a stale id (one that got applied/dismissed
// elsewhere) drops out without touching the rest of the selection.
const mdbRepairSel = new Set<number>();
// Per-row apply failure message, transient (never persisted) — cleared when the row is
// reselected or the next apply_repairs batch touches it again.
const mdbErrorById = new Map<number, string>();
// M8 Tier 2 playlist-dedup section state — stateless on the backend (no server-side id, see the
// IPC wiring plan's Architecture note), so the frontend keeps the last scan result itself and
// references entries by array index from the DOM. Re-populated on every renderRekordboxLive()
// call. Only ever reassigned here (renderRekordboxLive) — the click handler in sift-live.ts only
// reads it by index, which is a live-binding read and works fine through the import.
let lastScannedDuplicateGroups: PlaylistDuplicateGroupDto[] = [];
// Per-group dedup failure message, keyed by "playlistId::contentId" (no numeric id exists for a
// duplicate group) — same transient, never-persisted contract as mdbErrorById.
const mdbDedupErrorByKey = new Map<string, string>();
// M8 Tier 3 metadata-syncs section state — same module-level, filtered-not-reset discipline as
// mdbRepairSel.
const mdsSyncSel = new Set<number>();
const mdsErrorById = new Map<number, string>();
// M8 Tier 3 (pochette) artwork-syncs section state — same module-level, filtered-not-reset
// discipline as mdsSyncSel.
const masSyncSel = new Set<number>();
const masErrorById = new Map<number, string>();

// Session-group expand/collapse state for the 3 M8 candidate sections — groups are collapsed by
// default (nothing in the set), same module-level/filtered-not-reset discipline as the Sel sets
// above. Keyed by `session_id ?? SESSION_GROUP_NONE` (a real session_id can't collide with this
// sentinel since Sift's session ids are timestamp-based numeric strings).
const SESSION_GROUP_NONE = "__none__";
const mdbExpandedGroups = new Set<string>();
const mdsExpandedGroups = new Set<string>();
const masExpandedGroups = new Set<string>();

// Last-rendered *pending* rows per section, refreshed at the top of each section's render call —
// same "cache so the delegated click handler can read it synchronously" pattern as
// lastScannedDuplicateGroups above. A group-select click needs the full id list for its
// session_id at click time, before the next renderRekordboxLive() refetch resolves.
let lastPendingRepairs: PendingMasterdbRepair[] = [];
let lastPendingMetadataSyncs: PendingMetadataSync[] = [];
let lastPendingArtworkSyncs: PendingArtworkSync[] = [];

/** Cached from the last renderRekordboxLive() full render — lets the 4 sync section
 *  functions (masterdbRepairsSectionHtml, metadataSyncsSectionHtml, artworkSyncsSectionHtml,
 *  playlistDuplicatesSectionHtml) know whether the XML link itself is broken, so their idle
 *  state doesn't claim "à jour" when synchronization is actually unavailable (finding F3,
 *  audit-heuristique-visuel.md). null until the first render. */
let lastLinkStatus: RekordboxLinkStatus | null = null;

/** Une carte de synchronisation ne peut rien faire dans DEUX cas, pas un.
 *
 *  Impasse A13 (issue #15) : les quatre appels ne testaient que `lastLinkStatus.error`, qui ne
 *  parle que du XML lié. Les détecteurs M8, eux, lisent `master.db` — absent sur toute machine
 *  sans Rekordbox, et leurs erreurs deviennent des `None` muets côté Rust. Le corps de carte
 *  restait donc vide, et un corps vide se peint « à jour ». L'app affirmait que tout était
 *  synchronisé exactement quand rien ne pouvait l'être. */
function syncUnavailable(): boolean {
  return lastLinkStatus?.error != null || lastLinkStatus?.masterdb_error != null;
}

/** Ids of every pending row in `rows` whose `session_id` matches `sessionKey`
 * (`SESSION_GROUP_NONE` for null) — shared by the 3 group-select click handlers in sift-live.ts. */
function idsInSessionGroup<T extends { id: number; session_id: string | null }>(
  rows: T[],
  sessionKey: string,
): number[] {
  return rows.filter((r) => (r.session_id ?? SESSION_GROUP_NONE) === sessionKey).map((r) => r.id);
}

/** Groups pending rows by `session_id` (insertion order, `SESSION_GROUP_NONE` for null — pre-v8
 * rows), mirroring journal.ts's session-grouping convention (`sessionGroupHtml`). Shared by the 3
 * M8 candidate sections instead of tripling the loop. */
function groupBySession<T extends { id: number; session_id: string | null }>(rows: T[]): Map<string, T[]> {
  const map = new Map<string, T[]>();
  for (const r of rows) {
    const key = r.session_id ?? SESSION_GROUP_NONE;
    if (!map.has(key)) map.set(key, []);
    map.get(key)!.push(r);
  }
  return map;
}

/** Renders one collapsible session-group wrapper: header (label, count, expand toggle, "tout
 * sélectionner/désélectionner pour cette session") + its rows when expanded. `groupAction` and
 * `toggleAction` are the `data-sift` values sift-live.ts's delegated handler dispatches on
 * (`mdb*`/`mds*`/`mas*`, one pair per section — see the click handling note at the top of this
 * file for why the mutation itself lives there, not here). */
function sessionGroupHtml<T extends { id: number; session_id: string | null }>(
  sessionKey: string,
  rows: T[],
  sel: Set<number>,
  expanded: Set<string>,
  toggleAction: string,
  groupAction: string,
  rowHtml: (r: T) => string,
): string {
  const isOpen = expanded.has(sessionKey);
  // Le nom du Journal (« Session du 08/09/2026 01h24 »), pas l'identifiant brut — une session porte
  // un seul nom dans l'app (décision du 2026-09-08). « Antérieur » = lignes d'avant le schéma v8,
  // sans session : ce n'est pas le « Hors session » du Journal, qui date d'une session absente.
  const label = sessionKey === SESSION_GROUP_NONE ? "Antérieur" : sessionLabel(sessionKey, true);
  const allSelected = rows.length > 0 && rows.every((r) => sel.has(r.id));
  return (
    `<div class="rb-session-group">` +
    `<div class="rb-session-hd">` +
    `<button data-sift="${toggleAction}" data-session="${esc(sessionKey)}" class="rb-session-toggle" aria-expanded="${isOpen}">` +
    `${isOpen ? "▾" : "▸"} ${esc(label)} (${rows.length})</button>` +
    `<button data-sift="${groupAction}" data-session="${esc(sessionKey)}" class="rb-session-selectall">` +
    `${allSelected ? "Tout désélectionner" : "Tout sélectionner"}</button>` +
    `</div>` +
    (isOpen ? rows.map(rowHtml).join("") : "") +
    `</div>`
  );
}

function duplicateGroupKey(g: PlaylistDuplicateGroupDto): string {
  return `${g.playlist_id}::${g.content_id}`;
}

/** Busy state for the 4 master.db write buttons, class for class on the repo's own precedent
 *  (filing-actions.ts's doRanger: `ti ti-loader-2 sift-spin sift-icon-inline-md` + a label). These
 *  writes take seconds and used to show one static word ("Application…"), which reads as a frozen
 *  button rather than as work in progress.
 *  `text` is a LABEL — what the button is doing, counted — never an instruction: "close Rekordbox
 *  first" belongs to the confirm dialog that precedes the click, and a button that has already been
 *  pressed is the one place where telling the user to do something is useless.
 *  Only numbers and literals reach this, so the interpolation carries no user input. */
function busyLabel(text: string): string {
  return `<i class="ti ti-loader-2 sift-spin sift-icon-inline-md"></i> ${text}`;
}

/** Fallback card for a M8 section whose IPC call threw in renderRekordboxLive — replaces the
 * previous silent "" (section vanishes with no message, audit UX finding). Callers must also
 * reset that section's `lastPending*` array to [] so the umbrella pending count above the 4
 * cards stays consistent with what's actually shown. */
function sectionErrorHtml(): string {
  // Impasse A15 (issue #15) : « réessaie plus tard » était dit à une condition PERMANENTE. Sur une
  // machine sans Rekordbox, `master.db` ne réapparaîtra pas tout seul — réessayer ne changera
  // jamais rien, et le message envoyait l'utilisateur attendre. Quand la cause est connue on la
  // nomme ; le conseil d'attendre ne subsiste que pour ce qui est vraiment transitoire.
  const known = lastLinkStatus?.masterdb_error;
  const msg = known
    ? `${known} — la synchronisation Rekordbox reste indisponible tant qu'il manque.`
    : "Impossible de charger — réessaie plus tard.";
  return (
    `<div class="rb-row">` +
    `<div style="font-size:var(--text-sm);color:var(--color-text-danger)">${esc(msg)}</div>` +
    `</div>`
  );
}

/** Grammaire de FICHE des quatre sections (Fichiers, Métadonnées, Pochettes, Playlists) — celle des
 *  fiches de Revue (`.sift-meta-header` / `.sift-meta-title`, direction T), SANS cadre : la
 *  référence (Utilitaire de disque, guide Apple) pose sous la cible une grille d'informations, pas
 *  des boîtes (décision du 2026-09-08, déclinaison #24, maquette v2 validée). À droite : le compte,
 *  ou l'état. Une seule forme pour les quatre, pour que l'écran se lise comme une file.
 *  `body` vaut "" quand rien n'est en attente : la fiche reste (atténuée, « à jour ») au lieu de
 *  disparaître, donc les quatre sections n'apparaissent ni ne disparaissent au gré des comptes —
 *  décision du 2026-07-11, inchangée. */
function syncCardHtml(title: string, count: number, body: string, unavailable: boolean): string {
  const idle = body === "";
  const right = idle
    ? `<span class="rb-fiche-state">${unavailable ? "indisponible" : "à jour"}</span>`
    : `<span class="rb-fiche-count">${count}</span>`;
  return (
    `<div class="rb-fiche${idle ? " rb-fiche--idle" : ""}">` +
    `<div class="sift-meta-header"><span class="sift-meta-title">${esc(title)}</span>${right}</div>` +
    body +
    `</div>`
  );
}

/** Ce que le badge de la cible affiche — trois états et non deux (2026-08-17) : « à jour » n'est
 *  dit que quand les quatre sections ont RÉPONDU et qu'aucune n'a rien en attente. Zéro sur un
 *  écran cassé n'est pas zéro, c'est une absence de réponse (impasses A13 et A14, issue #15). */
interface TargetBadge {
  n: string;
  caption: string;
  warn: boolean;
}

/** Tête de zone C = la CIBLE, grammaire d'Utilitaire de disque (guide Apple, zone centrale : nom en
 *  grand, deux sous-lignes, badge chiffré à droite et sa légende en capitales — « 2 TB · SHARED BY
 *  8 VOLUMES »). Ici la cible est le XML lié et le badge porte l'état de synchronisation ; le
 *  compte quitte la barre, la référence le met sur la cible. Les deux actions (Réexporter, Changer
 *  de XML) vivent dans la barre unifiée — la toolbar d'Utilitaire de disque (`mountBarActions`,
 *  renderRekordboxLive). Décision du 2026-09-08 ; c'était une carte bordée au-dessus de tout, avec
 *  ses deux boutons dedans. Appelée seulement pour `s.linked === true`. */
function targetHeadHtml(s: RekordboxLinkStatus, badge: TargetBadge): string {
  const path = s.path || "";
  const file = path.split(/[\\/]/).pop() || "XML Rekordbox";
  const sub = s.error
    ? `<div class="rkb-target-sub rkb-target-sub--danger">XML Rekordbox illisible — relie un fichier.</div>`
    : `<div class="rkb-target-sub">XML Rekordbox lié · ${s.playlist_count} playlists · ${s.track_count} pistes</div>`;
  // Cause connue de l'indisponibilité (master.db absent…) : nommée sous la cible, une fois, en
  // warning — plutôt que répétée dans chaque fiche.
  const note = s.masterdb_error ? `<div class="rkb-target-note">${esc(s.masterdb_error)}</div>` : "";
  return (
    `<div class="rkb-target">` +
    `<div class="rkb-target-body">` +
    `<div class="rkb-target-name">${esc(file)}</div>` +
    sub +
    `<div class="rkb-target-path">${esc(path)}</div>` +
    note +
    `</div>` +
    `<div class="rkb-badge${badge.warn ? " rkb-badge--warn" : ""}">` +
    `<div class="rkb-badge-n">${esc(badge.n)}</div>` +
    `<div class="rkb-badge-cap">${esc(badge.caption)}</div>` +
    `</div>` +
    `</div>`
  );
}

/** M8 Tier 1 section: lists master.db path-repair candidates detected passively at filing time
 * (`rekordbox_masterdb_repairs`, actions.rs::detect_masterdb_repair_if_linked) and lets the user
 * resolve/apply/dismiss them. Independent of `driftBanner` (XML repair signal, unrelated
 * mechanism) — see docs/superpowers/specs/2026-07-06-m8-tier1-ui-screen-design.md. Renders the
 * idle "à jour" card (via syncCardHtml) when there is nothing pending/ambiguous. */
function masterdbRepairsSectionHtml(rows: PendingMasterdbRepair[]): string {
  // Drop stale selection ids without touching the rest — same discipline as batchSel's own
  // re-filter in sift-live.ts.
  const liveIds = new Set(rows.map((r) => r.id));
  for (const id of [...mdbRepairSel]) if (!liveIds.has(id)) mdbRepairSel.delete(id);

  const ambiguous = rows.filter((r) => r.status === "ambiguous");
  const pending = rows.filter((r) => r.status === "pending");
  lastPendingRepairs = pending;

  const pathBlock = (r: PendingMasterdbRepair) =>
    `<div style="min-width:0;flex:1">` +
    `<div style="font-family:var(--font-mono);font-size:var(--text-sm);font-weight:500;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">${esc(r.to_path)}</div>` +
    `<div style="font-family:var(--font-mono);font-size:var(--text-xs);color:var(--color-text-tertiary);overflow:hidden;text-overflow:ellipsis;white-space:nowrap"><span style="opacity:.55">was</span> ${esc(r.from_path)}</div>` +
    (mdbErrorById.has(r.id)
      ? `<div style="font-size:var(--text-xs);color:var(--color-text-danger);margin-top:2px">${esc(mdbErrorById.get(r.id)!)}</div>`
      : "") +
    `</div>`;

  const candidateList = (r: PendingMasterdbRepair): CandidateTrack[] =>
    r.candidate_tracks && r.candidate_tracks.length
      ? r.candidate_tracks
      : (r.candidate_track_ids || "")
          .split(",")
          .filter(Boolean)
          .map((track_id) => ({ track_id, folder_path: null }));

  const ambiguousRows = ambiguous
    .map((r) => {
      const candidateBtns = candidateList(r)
        .map(
          (c) =>
            `<button data-sift="mdbresolve" data-id="${r.id}" data-track="${esc(c.track_id)}" style="display:block;text-align:left;font-family:var(--font-mono);font-size:var(--text-xs)">` +
            `Choisir cette piste — ${esc(c.folder_path || c.track_id)}</button>`,
        )
        .join("");
      return (
        `<div class="rb-row">` +
        `<div style="display:flex;gap:10px;align-items:flex-start">${pathBlock(r)}` +
        `<button data-sift="mdbdismiss" data-id="${r.id}" style="flex:none">Ignorer</button></div>` +
        `<div style="margin-top:6px;display:flex;flex-direction:column;gap:3px">${candidateBtns}</div>` +
        `</div>`
      );
    })
    .join("");

  const pendingRowHtml = (r: PendingMasterdbRepair) => {
    const checked = mdbRepairSel.has(r.id);
    return (
      // Audit-ref G3 (Rekordbox, 2026-07-09) : ligne-checkbox sans clavier — tabindex/role/
      // aria-checked ajoutés, clavier via installNavKeyboard() étendu. Le bouton "Ignorer" imbriqué
      // est déjà protégé par la garde anti-double-déclenchement (Bibliothèque, audit-ref B1).
      `<div class="bx-row" data-sift="mdbpick" data-id="${r.id}" tabindex="0" role="checkbox" aria-checked="${checked}" style="display:flex;align-items:center;gap:var(--space-8);padding:var(--space-8);border-radius:var(--border-radius-md);cursor:pointer;${
        checked ? "background:var(--overlay-hover)" : ""
      }">` +
      `<input type="checkbox" class="sift-batch-ck" ${checked ? "checked" : ""} tabindex="-1">` +
      pathBlock(r) +
      `<button data-sift="mdbdismiss" data-id="${r.id}" style="flex:none">Ignorer</button>` +
      `</div>`
    );
  };

  const pendingRows = [...groupBySession(pending).entries()]
    .map(([sid, rows]) => sessionGroupHtml(sid, rows, mdbRepairSel, mdbExpandedGroups, "mdbgrouptoggle", "mdbgroupselect", pendingRowHtml))
    .join("");

  const applyBar =
    mdbRepairSel.size > 0
      ? `<div style="margin-top:8px"><button data-sift="mdbapply" class="sift-ranger-btn">Appliquer la sélection (${mdbRepairSel.size})</button></div>`
      : "";

  const subtext =
    pending.length > 0
      ? `<div style="font-size:var(--text-xs);color:var(--color-text-tertiary);margin-bottom:6px">${pending.length} morceau${pending.length > 1 ? "x" : ""} à synchroniser</div>`
      : "";

  const body =
    ambiguous.length === 0 && pending.length === 0
      ? ""
      : subtext + (ambiguousRows ? `<div style="margin-bottom:8px">${ambiguousRows}</div>` : "") + pendingRows + applyBar;

  return `<div id="sift-rkb-masterdb-section">${syncCardHtml("Fichiers", pending.length, body, syncUnavailable())}</div>`;
}

/** Re-renders only the Tier 1 repairs section from already-cached data (`lastPendingRepairs`),
 * for actions that mutate purely local UI state (row/group selection, group expand/collapse) and
 * touch nothing on the backend — no IPC re-fetch, no master.db re-read, no rebuild of the other 3
 * page sections. Falls back to a full `renderRekordboxLive()` if the section isn't in the DOM
 * (e.g. the page was just opened and hasn't rendered it yet). Click handling stays correct because
 * `[data-sift]` clicks are delegated once on `#pa` (installLiveWiring), not bound per-element. */
function rerenderMasterdbRepairsSection(): void {
  const el = document.getElementById("sift-rkb-masterdb-section");
  if (!el) {
    void renderRekordboxLive();
    return;
  }
  el.outerHTML = masterdbRepairsSectionHtml(lastPendingRepairs);
}

/** M8 Tier 3 section: lists master.db metadata sync candidates detected passively whenever Sift
 * writes ID3 tags on a file linked to Rekordbox (filing, "Appliquer les tags", édition
 * Bibliothèque — see docs/superpowers/plans/2026-07-09-m8-tier3-metadata-sync-ipc-ui.md).
 * Independent of masterdbRepairsSectionHtml/playlistDuplicatesSectionHtml — 3 separate sections,
 * never merged. Renders the idle "à jour" card (via syncCardHtml) when nothing pending/ambiguous. */
function metadataSyncsSectionHtml(rows: PendingMetadataSync[]): string {
  const liveIds = new Set(rows.map((r) => r.id));
  for (const id of [...mdsSyncSel]) if (!liveIds.has(id)) mdsSyncSel.delete(id);

  const ambiguous = rows.filter((r) => r.status === "ambiguous");
  const pending = rows.filter((r) => r.status === "pending");
  lastPendingMetadataSyncs = pending;

  const diffLine = (label: string, value: string | number | null) =>
    value == null ? "" : `<div style="font-size:var(--text-xs);color:var(--color-text-tertiary)">${label}: ${esc(String(value))}</div>`;

  const infoBlock = (r: PendingMetadataSync) =>
    `<div style="min-width:0;flex:1">` +
    `<div style="font-family:var(--font-mono);font-size:var(--text-sm);overflow:hidden;text-overflow:ellipsis;white-space:nowrap">${esc(r.sift_path)}</div>` +
    diffLine("Artiste", r.new_artist) +
    diffLine("Titre", r.new_title) +
    diffLine("Label", r.new_label) +
    diffLine("Année", r.new_year) +
    diffLine("Genre", r.new_genre) +
    (mdsErrorById.has(r.id)
      ? `<div style="font-size:var(--text-xs);color:var(--color-text-danger);margin-top:2px">${esc(mdsErrorById.get(r.id)!)}</div>`
      : "") +
    `</div>`;

  const candidateList = (r: PendingMetadataSync): CandidateTrack[] =>
    r.candidate_tracks && r.candidate_tracks.length
      ? r.candidate_tracks
      : (r.candidate_track_ids || "")
          .split(",")
          .filter(Boolean)
          .map((track_id) => ({ track_id, folder_path: null }));

  const ambiguousRows = ambiguous
    .map((r) => {
      const candidateBtns = candidateList(r)
        .map(
          (c) =>
            `<button data-sift="mdsresolve" data-id="${r.id}" data-track="${esc(c.track_id)}" style="display:block;text-align:left;font-family:var(--font-mono);font-size:var(--text-xs)">` +
            `Choisir cette piste — ${esc(c.folder_path || c.track_id)}</button>`,
        )
        .join("");
      return (
        `<div class="rb-row">` +
        `<div style="display:flex;gap:10px;align-items:flex-start">${infoBlock(r)}` +
        `<button data-sift="mdsdismiss" data-id="${r.id}" style="flex:none">Ignorer</button></div>` +
        `<div style="margin-top:6px;display:flex;flex-direction:column;gap:3px">${candidateBtns}</div>` +
        `</div>`
      );
    })
    .join("");

  const pendingRowHtml = (r: PendingMetadataSync) => {
    const checked = mdsSyncSel.has(r.id);
    return (
      `<div class="bx-row" data-sift="mdspick" data-id="${r.id}" tabindex="0" role="checkbox" aria-checked="${checked}" style="display:flex;align-items:center;gap:var(--space-8);padding:var(--space-8);border-radius:var(--border-radius-md);cursor:pointer;${
        checked ? "background:var(--overlay-hover)" : ""
      }">` +
      `<input type="checkbox" class="sift-batch-ck" ${checked ? "checked" : ""} tabindex="-1">` +
      infoBlock(r) +
      `<button data-sift="mdsdismiss" data-id="${r.id}" style="flex:none">Ignorer</button>` +
      `</div>`
    );
  };

  const pendingRows = [...groupBySession(pending).entries()]
    .map(([sid, rows]) => sessionGroupHtml(sid, rows, mdsSyncSel, mdsExpandedGroups, "mdsgrouptoggle", "mdsgroupselect", pendingRowHtml))
    .join("");

  const applyBar =
    mdsSyncSel.size > 0
      ? `<div style="margin-top:8px"><button data-sift="mdsapply" class="sift-ranger-btn">Appliquer la sélection (${mdsSyncSel.size})</button></div>`
      : "";

  const subtext =
    pending.length > 0
      ? `<div style="font-size:var(--text-xs);color:var(--color-text-tertiary);margin-bottom:6px">${pending.length} morceau${pending.length > 1 ? "x" : ""} à synchroniser</div>`
      : "";

  const body =
    ambiguous.length === 0 && pending.length === 0
      ? ""
      : subtext + (ambiguousRows ? `<div style="margin-bottom:8px">${ambiguousRows}</div>` : "") + pendingRows + applyBar;

  return `<div id="sift-rkb-mds-section">${syncCardHtml("Métadonnées", pending.length, body, syncUnavailable())}</div>`;
}

/** Same discipline as `rerenderMasterdbRepairsSection` for the Tier 3 metadata section. */
function rerenderMetadataSyncsSection(): void {
  const el = document.getElementById("sift-rkb-mds-section");
  if (!el) {
    void renderRekordboxLive();
    return;
  }
  el.outerHTML = metadataSyncsSectionHtml(lastPendingMetadataSyncs);
}

/** M8 Tier 3 (pochette) section: lists master.db artwork sync candidates detected passively
 * whenever Sift writes a NEW cover onto a file linked to Rekordbox. Independent of
 * metadataSyncsSectionHtml (separate table, separate detector — a text-only retag never lands
 * here). Renders the idle "à jour" card (via syncCardHtml) when nothing pending/ambiguous. */
function artworkSyncsSectionHtml(rows: PendingArtworkSync[]): string {
  const liveIds = new Set(rows.map((r) => r.id));
  for (const id of [...masSyncSel]) if (!liveIds.has(id)) masSyncSel.delete(id);

  const ambiguous = rows.filter((r) => r.status === "ambiguous");
  const pending = rows.filter((r) => r.status === "pending");
  lastPendingArtworkSyncs = pending;

  const coverFileName = (p: string) => p.split(/[\\/]/).pop() || p;

  const infoBlock = (r: PendingArtworkSync) =>
    `<div style="min-width:0;flex:1">` +
    `<div style="font-family:var(--font-mono);font-size:var(--text-sm);overflow:hidden;text-overflow:ellipsis;white-space:nowrap">${esc(r.sift_path)}</div>` +
    `<div style="font-size:var(--text-xs);color:var(--color-text-tertiary)">Nouvelle pochette : ${esc(coverFileName(r.cover_path))}</div>` +
    (masErrorById.has(r.id)
      ? `<div style="font-size:var(--text-xs);color:var(--color-text-danger);margin-top:2px">${esc(masErrorById.get(r.id)!)}</div>`
      : "") +
    `</div>`;

  const candidateList = (r: PendingArtworkSync): CandidateTrack[] =>
    r.candidate_tracks && r.candidate_tracks.length
      ? r.candidate_tracks
      : (r.candidate_track_ids || "")
          .split(",")
          .filter(Boolean)
          .map((track_id) => ({ track_id, folder_path: null }));

  const ambiguousRows = ambiguous
    .map((r) => {
      const candidateBtns = candidateList(r)
        .map(
          (c) =>
            `<button data-sift="masresolve" data-id="${r.id}" data-track="${esc(c.track_id)}" style="display:block;text-align:left;font-family:var(--font-mono);font-size:var(--text-xs)">` +
            `Choisir cette piste — ${esc(c.folder_path || c.track_id)}</button>`,
        )
        .join("");
      return (
        `<div class="rb-row">` +
        `<div style="display:flex;gap:10px;align-items:flex-start">${infoBlock(r)}` +
        `<button data-sift="masdismiss" data-id="${r.id}" style="flex:none">Ignorer</button></div>` +
        `<div style="margin-top:6px;display:flex;flex-direction:column;gap:3px">${candidateBtns}</div>` +
        `</div>`
      );
    })
    .join("");

  const pendingRowHtml = (r: PendingArtworkSync) => {
    const checked = masSyncSel.has(r.id);
    return (
      `<div class="bx-row" data-sift="maspick" data-id="${r.id}" tabindex="0" role="checkbox" aria-checked="${checked}" style="display:flex;align-items:center;gap:var(--space-8);padding:var(--space-8);border-radius:var(--border-radius-md);cursor:pointer;${
        checked ? "background:var(--overlay-hover)" : ""
      }">` +
      `<input type="checkbox" class="sift-batch-ck" ${checked ? "checked" : ""} tabindex="-1">` +
      infoBlock(r) +
      `<button data-sift="masdismiss" data-id="${r.id}" style="flex:none">Ignorer</button>` +
      `</div>`
    );
  };

  const pendingRows = [...groupBySession(pending).entries()]
    .map(([sid, rows]) => sessionGroupHtml(sid, rows, masSyncSel, masExpandedGroups, "masgrouptoggle", "masgroupselect", pendingRowHtml))
    .join("");

  const applyBar =
    masSyncSel.size > 0
      ? `<div style="margin-top:8px"><button data-sift="masapply" class="sift-ranger-btn">Appliquer la sélection (${masSyncSel.size})</button></div>`
      : "";

  const subtext =
    pending.length > 0
      ? `<div style="font-size:var(--text-xs);color:var(--color-text-tertiary);margin-bottom:6px">${pending.length} morceau${pending.length > 1 ? "x" : ""} à synchroniser</div>`
      : "";

  const body =
    ambiguous.length === 0 && pending.length === 0
      ? ""
      : subtext + (ambiguousRows ? `<div style="margin-bottom:8px">${ambiguousRows}</div>` : "") + pendingRows + applyBar;

  return `<div id="sift-rkb-mas-section">${syncCardHtml("Pochettes", pending.length, body, syncUnavailable())}</div>`;
}

/** Same discipline as `rerenderMasterdbRepairsSection` for the Tier 3 artwork section. */
function rerenderArtworkSyncsSection(): void {
  const el = document.getElementById("sift-rkb-mas-section");
  if (!el) {
    void renderRekordboxLive();
    return;
  }
  el.outerHTML = artworkSyncsSectionHtml(lastPendingArtworkSyncs);
}

/** M8 Tier 2 section: lists playlists where the same track appears more than once
 * (rekordbox_masterdb_scan_playlist_duplicates, read-only, scanned fresh on every render — no
 * persistence, see docs/superpowers/plans/2026-07-08-m8-tier2-ipc-wiring.md). One button per
 * group, no multi-select (unlike Tier 1's masterdbRepairsSectionHtml): each dedup is a complete,
 * independent action, and there are typically 0-2 groups at a time. Renders the idle "à jour" card
 * (via syncCardHtml) when there is nothing to dedup. */
function playlistDuplicatesSectionHtml(groups: PlaylistDuplicateGroupDto[]): string {
  const rows = groups
    .map((g, i) => {
      const key = duplicateGroupKey(g);
      const playlistLabel = g.playlist_name || `Playlist ${g.playlist_id}`;
      const trackLabel = g.track_path ? g.track_path.split(/[\\/]/).pop() || g.track_path : `Piste ${g.content_id}`;
      const count = g.remove.length;
      return (
        `<div class="rb-row rb-row--inline">` +
        `<div style="min-width:0;flex:1">` +
        `<div style="font-size:var(--text-sm)">${esc(playlistLabel)}</div>` +
        `<div style="font-family:var(--font-mono);font-size:var(--text-xs);color:var(--color-text-tertiary);overflow:hidden;text-overflow:ellipsis;white-space:nowrap">${esc(trackLabel)} — ${count} doublon${count > 1 ? "s" : ""}</div>` +
        (mdbDedupErrorByKey.has(key)
          ? `<div style="font-size:var(--text-xs);color:var(--color-text-danger);margin-top:2px">${esc(mdbDedupErrorByKey.get(key)!)}</div>`
          : "") +
        `</div>` +
        `<button data-sift="mdbdedup" data-idx="${i}" class="sift-ranger-btn" style="flex:none">Dédupliquer</button>` +
        `</div>`
      );
    })
    .join("");
  return syncCardHtml("Playlists", groups.length, rows, syncUnavailable());
}

/** Rekordbox integration page (data-view="rkb") — real screen replacing the old one-click nav
 * export (audit 2026-07-05, docs/superpowers/specs/2026-07-05-rekordbox-integration-page-design.md).
 * Renders the whole page fresh each call, same pattern as renderBiblioLive/renderJournal — no mock
 * DOM survives. `drift_detected` is independent of linked/error, so the banner can appear on top
 * of either linked state (never modeled as a 4-way exclusive if/else). */
export async function renderRekordboxLive(): Promise<void> {
  const content = requireEl("#content", "renderRekordboxLive");
  // Jeton capturé dans le même geste que `#content` (issue #42). Cet écran est le plus exposé de
  // tous : CINQ allers-retours IPC séquentiels avant sa première écriture complète, chacun bloqué
  // par le `Mutex<Connection>` que le scan tient en rafale.
  const token = viewEpoch();
  // Squelette statique au premier passage (DESIGN.md § 6) : cinq IPC séquentiels avant la
  // première peinture complète, et un `#content` vide pendant ce temps se lit « rien ». Un
  // re-rendu (après une action) garde l'écran précédent en place jusqu'à l'écriture.
  if (!content.querySelector(".sift-rkb-layout, .sift-empty-state")) {
    content.innerHTML =
      `<div class="sift-rkb-layout"><nav class="sift-rkb-side"></nav>` +
      `<div class="sift-rkb-main"><span class="sift-skel sift-skel-line"></span></div></div>`;
  }
  let status: RekordboxLinkStatus;
  try {
    status = await rekordboxStatus();
    lastLinkStatus = status;
  } catch (e) {
    console.error("rekordbox_status failed", e);
    if (isStaleViewRender(token)) return;
    mountBarActions("");
    content.innerHTML =
      `<div style="font-size:var(--text-md);color:var(--color-text-tertiary)">Statut Rekordbox indisponible.</div>`;
    return;
  }

  if (isStaleViewRender(token)) return;

  if (!status.linked) {
    // Pas de rappel de procédure ici : il ne vaut que pour un XML lié (spec § Zone A).
    mountBarActions("");
    content.innerHTML = emptyStateHtml({
      title: "Aucun XML Rekordbox lié",
      note: "Relie le fichier XML exporté depuis Rekordbox pour commencer à synchroniser tes conversions.",
      actionHtml: `<button data-bib="rkblink">Lier un fichier XML Rekordbox</button>`,
    });
    wireEmptyState(content);
    return;
  }

  // Barre unifiée = la toolbar d'Utilitaire de disque : les deux actions sur la cible, en texte
  // seul (CLAUDE.md § Front). Montée dès que le statut est connu, avant les quatre IPC des
  // sections. Pas de « Réexporter » tant que le fichier lié est illisible — le backend refuse
  // déjà l'export dans ce cas (export_rekordbox_xml_inner relit le même chemin avant de fusionner).
  mountBarActions(
    (status.error
      ? ""
      : `<button data-sift="rkbreexport" class="sift-ranger-btn sift-bar-btn">Réexporter maintenant</button>`) +
      `<button data-bib="rkblink" class="sift-bar-btn">Changer de XML lié</button>`,
  );

  // Rappel de procédure, sous la cible — pas un titre (spec § Zone A).
  const intro =
    `<div class="rkb-intro">` +
    `Sift convertit tes morceaux → l'export fusionne les nouveaux dans le XML lié → réimporte-le dans Rekordbox pour les voir apparaître.` +
    `</div>`;

  // Copy from the 2026-07-11 grill-me session: name the workflow explicitly (close Rekordbox
  // before touching the link — the same rule Tier 1/2/3 already enforce server-side via
  // MasterDbError::RekordboxRunning, see rekordbox_repairs.rs) instead of a vague "vérifie".
  const driftBanner = status.drift_detected
    ? `<div class="sift-dup-banner" style="background:var(--color-background-warning)">` +
      `<i class="ti ti-alert-triangle" style="color:var(--color-text-warning)"></i>` +
      `<div class="sift-dup-banner-body">` +
      `<div class="sift-dup-banner-head" style="color:var(--color-text-warning)">Une correction de chemin a échoué</div>` +
      // .sift-dup-banner-where is built for a truncated file path (nowrap+ellipsis) — this is a
      // full sentence, the entire payload of a warning that was previously invisible anywhere in
      // the UI, so it must never silently clip on a narrow window.
      `<div class="sift-dup-banner-where" style="white-space:normal;overflow:visible;text-overflow:clip">Ferme Rekordbox, vérifie la piste, puis relie à nouveau le fichier XML pour confirmer.</div>` +
      `</div></div>`
    : "";

  // Impasse A14 (issue #15) : les quatre `catch` remettent leur tableau à `[]` AVANT le calcul de
  // `totalPending` plus bas. Quatre cartes en erreur donnaient donc un total de 0, et un en-tête
  // qui annonçait « à jour » au-dessus d'elles. Compter les sections tombées est ce qui permet à
  // l'en-tête de dire autre chose que le total.
  let failedSections = 0;

  let masterdbSection = "";
  try {
    const repairs = await rekordboxMasterdbPendingRepairs();
    masterdbSection = masterdbRepairsSectionHtml(repairs);
  } catch (e) {
    console.error("rekordbox_masterdb_pending_repairs failed", e);
    lastPendingRepairs = [];
    failedSections++;
    masterdbSection = `<div id="sift-rkb-masterdb-section">${sectionErrorHtml()}</div>`;
  }

  let dedupSection = "";
  try {
    lastScannedDuplicateGroups = await rekordboxMasterdbScanPlaylistDuplicates();
    dedupSection = playlistDuplicatesSectionHtml(lastScannedDuplicateGroups);
  } catch (e) {
    console.error("rekordbox_masterdb_scan_playlist_duplicates failed", e);
    lastScannedDuplicateGroups = [];
    failedSections++;
    dedupSection = sectionErrorHtml();
  }

  let metadataSyncSection = "";
  try {
    const syncs = await rekordboxMasterdbPendingMetadataSyncs();
    metadataSyncSection = metadataSyncsSectionHtml(syncs);
  } catch (e) {
    console.error("rekordbox_masterdb_pending_metadata_syncs failed", e);
    lastPendingMetadataSyncs = [];
    failedSections++;
    metadataSyncSection = `<div id="sift-rkb-mds-section">${sectionErrorHtml()}</div>`;
  }

  let artworkSyncSection = "";
  try {
    const artworkSyncs = await rekordboxMasterdbPendingArtworkSyncs();
    artworkSyncSection = artworkSyncsSectionHtml(artworkSyncs);
  } catch (e) {
    console.error("rekordbox_masterdb_pending_artwork_syncs failed", e);
    lastPendingArtworkSyncs = [];
    failedSections++;
    artworkSyncSection = `<div id="sift-rkb-mas-section">${sectionErrorHtml()}</div>`;
  }

  // Le badge de la cible porte le total en attente des Tiers 1/2/3 — c'est lui qui fait lire les
  // quatre fiches comme UNE file (session grill-me du 2026-07-11) ; la sur-ligne « Synchroniser
  // avec Rekordbox » qui tenait ce rôle est partie avec la carte (2026-09-08).
  //
  // Trois états et non deux depuis le 2026-08-17 : « à jour » n'est dit que quand les quatre
  // sections ont RÉPONDU et qu'aucune n'a rien en attente. Zéro sur un écran cassé n'est pas
  // zéro — c'est une absence de réponse (impasses A13 et A14, issue #15). La cause connue
  // (`masterdb_error`) se lit sous la cible (targetHeadHtml), pas dans le badge.
  const totalPending = lastPendingRepairs.length + lastScannedDuplicateGroups.length + lastPendingMetadataSyncs.length + lastPendingArtworkSyncs.length;
  const badge: TargetBadge = syncUnavailable()
    ? { n: "—", caption: "synchronisation indisponible", warn: true }
    : failedSections > 0
      ? { n: "?", caption: `${failedSections} section${failedSections > 1 ? "s" : ""} sans réponse`, warn: true }
      : totalPending > 0
        ? { n: String(totalPending), caption: "en attente de synchronisation", warn: false }
        : { n: "0", caption: "à jour", warn: false };

  // QUATRE ENTRÉES, plus « Tout » — étape 10 (DESIGN.md § 17, spec `docs/ui-specs/rekordbox.md`).
  //
  // Les quatre sections M8 étaient quatre cartes EMPILÉES dans une page qui défilait, chacune avec
  // sa propre action. Quatre cibles et quatre actions dans un même flux vertical : rien ne disait
  // laquelle on traite. C'est exactement ce que le patron Utilitaire de disque règle — on choisit
  // une cible avant qu'une action existe. Elles deviennent des entrées de la zone gauche.
  //
  // Une section dont l'appel IPC a échoué GARDE son entrée, avec son compte remplacé par un
  // marqueur : une section absente se lirait « rien à faire », ce qui est un mensonge. C'est le
  // même raisonnement que les trois états de `syncState` ci-dessus.
  const sections: { key: string; label: string; html: string; count: number | null }[] = [
    { key: "files", label: "Fichiers", html: masterdbSection, count: lastPendingRepairs.length },
    { key: "meta", label: "Métadonnées", html: metadataSyncSection, count: lastPendingMetadataSyncs.length },
    { key: "art", label: "Pochettes", html: artworkSyncSection, count: lastPendingArtworkSyncs.length },
    { key: "dedup", label: "Playlists", html: dedupSection, count: lastScannedDuplicateGroups.length },
  ];
  if (!sections.some((x) => x.key === activeRkbSection) && activeRkbSection !== "all") activeRkbSection = "all";

  const entry = (key: string, label: string, count: number | null): string =>
    `<div class="fld${activeRkbSection === key ? " on" : ""}" data-rkb="section" data-sec="${key}" tabindex="0" role="button">` +
    `<span>${esc(label)}</span>` +
    `<span class="rkb-entry-count">${count == null ? "—" : count}</span></div>`;

  // La colonne des sections est la sidebar d'Utilitaire de disque : au PLAN DE LA FILE de Revue
  // (`--color-background-queue`, bord à bord), plus une carte — Antoine, 2026-09-08.
  const side =
    `<nav class="sift-rkb-side" aria-label="Sections de synchronisation">` +
    `<div class="col-h">Synchroniser</div>` +
    entry("all", "Tout", totalPending) +
    sections.map((x) => entry(x.key, x.label, x.count)).join("") +
    `</nav>`;

  const body =
    activeRkbSection === "all"
      ? sections.map((x) => x.html).join("")
      : (sections.find((x) => x.key === activeRkbSection)?.html ?? "");

  if (isStaleViewRender(token)) return;
  // Zone C, de haut en bas : le bandeau de dérive (spec § États, en tête, jamais tronqué), la
  // cible, le rappel de procédure, puis les fiches de la section choisie.
  content.innerHTML =
    `<div class="sift-rkb-layout">${side}` +
    `<div class="sift-rkb-main">${driftBanner}${targetHeadHtml(status, badge)}${intro}${body}</div>` +
    `</div>`;
}

/** Section affichée. Au niveau module, comme les quatre tableaux d'état au-dessus : l'écran se
 *  re-rend après chaque synchronisation, et un état local ramènerait l'utilisateur sur « Tout »
 *  juste après qu'il ait choisi une cible. */
let activeRkbSection = "all";

/** Appelée par le dispatch délégué au clic sur une entrée de section. */
export function onRekordboxSectionPick(key: string): void {
  activeRkbSection = key;
  void renderRekordboxLive();
}

/** Routes the Rekordbox master.db action panel's delegated clicks (Tier 1 path repairs, Tier 3
 *  metadata/artwork sync — the `rkbreexport`/`mdb*`/`mds*`/`mas*` `data-sift` actions). Extracted
 *  from sift-live.ts's installLiveWiring click handler (Phase 1, tranche 1a) — this state already
 *  lived here, the dispatch logic follows it. Returns true if it handled `act` (caller must stop
 *  processing), false otherwise so the caller's chain can continue to non-Rekordbox actions.
 *  `onReexport` is injected because the actual XML export (`runNavExport`) stays in sift-live.ts —
 *  this avoids a reverse import back into sift-live.ts. */
export function handleRekordboxAction(
  el: HTMLElement,
  act: string,
  e: MouseEvent,
  onReexport: () => void,
): boolean {
  if (act === "rkbreexport") {
    e.stopPropagation();
    onReexport();
  } else if (act === "mdbpick") {
    e.stopPropagation();
    const id = Number(el.dataset.id);
    if (mdbRepairSel.has(id)) {
      mdbRepairSel.delete(id);
    } else {
      mdbRepairSel.add(id);
      mdbErrorById.delete(id);
    }
    rerenderMasterdbRepairsSection();
  } else if (act === "mdbgrouptoggle") {
    e.stopPropagation();
    const key = el.dataset.session || "";
    if (mdbExpandedGroups.has(key)) mdbExpandedGroups.delete(key);
    else mdbExpandedGroups.add(key);
    rerenderMasterdbRepairsSection();
  } else if (act === "mdbgroupselect") {
    e.stopPropagation();
    const key = el.dataset.session || "";
    const ids = idsInSessionGroup(lastPendingRepairs, key);
    const allSelected = ids.length > 0 && ids.every((id) => mdbRepairSel.has(id));
    for (const id of ids) {
      if (allSelected) mdbRepairSel.delete(id);
      else {
        mdbRepairSel.add(id);
        mdbErrorById.delete(id);
      }
    }
    rerenderMasterdbRepairsSection();
  } else if (act === "mdbdismiss") {
    e.stopPropagation();
    const id = Number(el.dataset.id);
    void (async () => {
      try {
        await rekordboxMasterdbDismissRepair(id);
      } catch (e) {
        console.error("rekordbox_masterdb_dismiss_repair failed", e);
        toast("Action impossible — réessaie");
      }
      void renderRekordboxLive();
    })();
  } else if (act === "mdbresolve") {
    e.stopPropagation();
    const id = Number(el.dataset.id);
    const trackId = el.dataset.track || "";
    void (async () => {
      try {
        await rekordboxMasterdbResolveAmbiguous(id, trackId);
      } catch (e) {
        console.error("rekordbox_masterdb_resolve_ambiguous failed", e);
        const raw = String(e);
        // Ces deux messages viennent tels quels du backend (rekordbox_repairs.rs
        // resolve_ambiguous_inner) — déjà humains, pas fabriqués ici.
        toast(
          raw.includes("plus ambiguë") || raw.includes("piste choisie invalide")
            ? raw
            : "Choix impossible — réessaie",
        );
      }
      void renderRekordboxLive();
    })();
  } else if (act === "mdbapply") {
    e.stopPropagation();
    const ids = [...mdbRepairSel];
    if (!ids.length) return true;
    const btn = el as HTMLButtonElement;
    if (btn.disabled) return true;
    void (async () => {
      const proceed = await confirmAction(
        `Synchroniser ${ids.length} fichier${ids.length > 1 ? "s" : ""} avec Rekordbox ? Ferme Rekordbox avant de continuer.`,
        "Synchroniser",
      );
      if (!proceed) return;
      btn.disabled = true;
      btn.innerHTML = busyLabel(
        `Synchronisation de ${ids.length} fichier${ids.length > 1 ? "s" : ""}…`,
      );
      try {
        const outcomes = await rekordboxMasterdbApplyRepairs(ids);
        let ok = 0;
        for (const o of outcomes) {
          mdbRepairSel.delete(o.id);
          if (o.ok) {
            mdbErrorById.delete(o.id);
            ok++;
          } else {
            mdbErrorById.set(o.id, o.error || "échec inconnu");
          }
        }
        const failed = outcomes.length - ok;
        toast(
          failed > 0
            ? `${ok} fichier${ok > 1 ? "s" : ""} synchronisé${ok > 1 ? "s" : ""}, ${failed} échoué${failed > 1 ? "s" : ""}`
            : `${ok} fichier${ok > 1 ? "s" : ""} synchronisé${ok > 1 ? "s" : ""}`,
        );
      } catch (e) {
        console.error("rekordbox_masterdb_apply_repairs failed", e);
        toast("Action impossible — réessaie");
      }
      void renderRekordboxLive();
    })();
  } else if (act === "mdbdedup") {
    e.stopPropagation();
    const idx = Number(el.dataset.idx);
    const group = lastScannedDuplicateGroups[idx];
    if (!group) return true;
    const btn = el as HTMLButtonElement;
    if (btn.disabled) return true;
    void (async () => {
      const proceed = await confirmAction(
        `Synchroniser cette playlist avec Rekordbox — retirer ${group.remove.length} doublon${group.remove.length > 1 ? "s" : ""} ? Ferme Rekordbox avant de continuer.`,
        "Synchroniser",
      );
      if (!proceed) return;
      btn.disabled = true;
      btn.innerHTML = busyLabel(
        `Retrait de ${group.remove.length} doublon${group.remove.length > 1 ? "s" : ""}…`,
      );
      const key = duplicateGroupKey(group);
      try {
        await rekordboxMasterdbDedupPlaylistGroup(group);
        mdbDedupErrorByKey.delete(key);
        toast(`${group.remove.length} doublon${group.remove.length > 1 ? "s" : ""} retiré${group.remove.length > 1 ? "s" : ""}`);
      } catch (e) {
        console.error("rekordbox_masterdb_dedup_playlist_group failed", e);
        mdbDedupErrorByKey.set(key, e instanceof Error ? e.message : "échec inconnu");
      }
      void renderRekordboxLive();
    })();
  } else if (act === "mdspick") {
    e.stopPropagation();
    const id = Number(el.dataset.id);
    if (mdsSyncSel.has(id)) {
      mdsSyncSel.delete(id);
    } else {
      mdsSyncSel.add(id);
      mdsErrorById.delete(id);
    }
    rerenderMetadataSyncsSection();
  } else if (act === "mdsgrouptoggle") {
    e.stopPropagation();
    const key = el.dataset.session || "";
    if (mdsExpandedGroups.has(key)) mdsExpandedGroups.delete(key);
    else mdsExpandedGroups.add(key);
    rerenderMetadataSyncsSection();
  } else if (act === "mdsgroupselect") {
    e.stopPropagation();
    const key = el.dataset.session || "";
    const ids = idsInSessionGroup(lastPendingMetadataSyncs, key);
    const allSelected = ids.length > 0 && ids.every((id) => mdsSyncSel.has(id));
    for (const id of ids) {
      if (allSelected) mdsSyncSel.delete(id);
      else {
        mdsSyncSel.add(id);
        mdsErrorById.delete(id);
      }
    }
    rerenderMetadataSyncsSection();
  } else if (act === "mdsdismiss") {
    e.stopPropagation();
    const id = Number(el.dataset.id);
    void (async () => {
      try {
        await rekordboxMasterdbDismissMetadataSync(id);
      } catch (e) {
        console.error("rekordbox_masterdb_dismiss_metadata_sync failed", e);
        toast("Action impossible — réessaie");
      }
      void renderRekordboxLive();
    })();
  } else if (act === "mdsresolve") {
    e.stopPropagation();
    const id = Number(el.dataset.id);
    const trackId = el.dataset.track || "";
    void (async () => {
      try {
        await rekordboxMasterdbResolveAmbiguousMetadataSync(id, trackId);
      } catch (e) {
        console.error("rekordbox_masterdb_resolve_ambiguous_metadata_sync failed", e);
        const raw = String(e);
        // Ces deux messages viennent tels quels du backend (rekordbox_repairs.rs
        // resolve_ambiguous_metadata_sync_inner) — déjà humains, pas fabriqués ici.
        toast(
          raw.includes("plus ambiguë") || raw.includes("piste choisie invalide")
            ? raw
            : "Choix impossible — réessaie",
        );
      }
      void renderRekordboxLive();
    })();
  } else if (act === "mdsapply") {
    e.stopPropagation();
    const ids = [...mdsSyncSel];
    if (!ids.length) return true;
    const btn = el as HTMLButtonElement;
    if (btn.disabled) return true;
    void (async () => {
      const proceed = await confirmAction(
        `Synchroniser les métadonnées de ${ids.length} morceau${ids.length > 1 ? "x" : ""} avec Rekordbox ? Ferme Rekordbox avant de continuer.`,
        "Synchroniser",
      );
      if (!proceed) return;
      btn.disabled = true;
      btn.innerHTML = busyLabel(
        `Synchronisation de ${ids.length} morceau${ids.length > 1 ? "x" : ""}…`,
      );
      try {
        const outcomes: ApplyMetadataSyncOutcome[] = await rekordboxMasterdbApplyMetadataSyncs(ids);
        let ok = 0;
        for (const o of outcomes) {
          mdsSyncSel.delete(o.id);
          if (o.ok) {
            mdsErrorById.delete(o.id);
            ok++;
          } else {
            mdsErrorById.set(o.id, o.error || "échec inconnu");
          }
        }
        const failed = outcomes.length - ok;
        toast(
          failed > 0
            ? `${ok} morceau${ok > 1 ? "x" : ""} synchronisé${ok > 1 ? "s" : ""}, ${failed} échoué${failed > 1 ? "s" : ""}`
            : `${ok} morceau${ok > 1 ? "x" : ""} synchronisé${ok > 1 ? "s" : ""}`,
        );
      } catch (e) {
        console.error("rekordbox_masterdb_apply_metadata_syncs failed", e);
        toast("Action impossible — réessaie");
      }
      void renderRekordboxLive();
    })();
  } else if (act === "maspick") {
    e.stopPropagation();
    const id = Number(el.dataset.id);
    if (masSyncSel.has(id)) {
      masSyncSel.delete(id);
    } else {
      masSyncSel.add(id);
      masErrorById.delete(id);
    }
    rerenderArtworkSyncsSection();
  } else if (act === "masgrouptoggle") {
    e.stopPropagation();
    const key = el.dataset.session || "";
    if (masExpandedGroups.has(key)) masExpandedGroups.delete(key);
    else masExpandedGroups.add(key);
    rerenderArtworkSyncsSection();
  } else if (act === "masgroupselect") {
    e.stopPropagation();
    const key = el.dataset.session || "";
    const ids = idsInSessionGroup(lastPendingArtworkSyncs, key);
    const allSelected = ids.length > 0 && ids.every((id) => masSyncSel.has(id));
    for (const id of ids) {
      if (allSelected) masSyncSel.delete(id);
      else {
        masSyncSel.add(id);
        masErrorById.delete(id);
      }
    }
    rerenderArtworkSyncsSection();
  } else if (act === "masdismiss") {
    e.stopPropagation();
    const id = Number(el.dataset.id);
    void (async () => {
      try {
        await rekordboxMasterdbDismissArtworkSync(id);
      } catch (e) {
        console.error("rekordbox_masterdb_dismiss_artwork_sync failed", e);
        toast("Action impossible — réessaie");
      }
      void renderRekordboxLive();
    })();
  } else if (act === "masresolve") {
    e.stopPropagation();
    const id = Number(el.dataset.id);
    const trackId = el.dataset.track || "";
    void (async () => {
      try {
        await rekordboxMasterdbResolveAmbiguousArtworkSync(id, trackId);
      } catch (e) {
        console.error("rekordbox_masterdb_resolve_ambiguous_artwork_sync failed", e);
        const raw = String(e);
        // Ces deux messages viennent tels quels du backend (rekordbox_repairs.rs
        // rekordbox_masterdb_resolve_ambiguous_artwork_sync_inner) — déjà humains, pas fabriqués ici.
        toast(
          raw.includes("plus ambiguë") || raw.includes("piste choisie invalide")
            ? raw
            : "Choix impossible — réessaie",
        );
      }
      void renderRekordboxLive();
    })();
  } else if (act === "masapply") {
    e.stopPropagation();
    const ids = [...masSyncSel];
    if (!ids.length) return true;
    const btn = el as HTMLButtonElement;
    if (btn.disabled) return true;
    void (async () => {
      const proceed = await confirmAction(
        `Synchroniser la pochette de ${ids.length} morceau${ids.length > 1 ? "x" : ""} avec Rekordbox ? Ferme Rekordbox avant de continuer.`,
        "Synchroniser",
      );
      if (!proceed) return;
      btn.disabled = true;
      btn.innerHTML = busyLabel(
        `Synchronisation de ${ids.length} pochette${ids.length > 1 ? "s" : ""}…`,
      );
      try {
        const outcomes = await rekordboxMasterdbApplyArtworkSyncs(ids);
        let ok = 0;
        for (const o of outcomes) {
          masSyncSel.delete(o.id);
          if (o.ok) {
            masErrorById.delete(o.id);
            ok++;
          } else {
            masErrorById.set(o.id, o.error || "échec inconnu");
          }
        }
        const failed = outcomes.length - ok;
        toast(
          failed > 0
            ? `${ok} pochette${ok > 1 ? "s" : ""} synchronisée${ok > 1 ? "s" : ""}, ${failed} échouée${failed > 1 ? "s" : ""}`
            : `${ok} pochette${ok > 1 ? "s" : ""} synchronisée${ok > 1 ? "s" : ""}`,
        );
      } catch (e) {
        console.error("rekordbox_masterdb_apply_artwork_syncs failed", e);
        toast("Action impossible — réessaie");
      }
      void renderRekordboxLive();
    })();
  } else {
    return false;
  }
  return true;
}
