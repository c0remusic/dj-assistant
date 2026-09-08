// Journal — le filet de sécurité du rangement (`docs/ui-specs/journal.md`).
//
// Patron macOS : Console. Une table d'événements horodatés, groupés, avec un détail à droite.
// C'est cet écran qui rend le rangement annulable, et c'est cette réversibilité qui autorise Revue
// à agir vite.
//
// Trois zones (DESIGN.md § 14) :
//   A — la barre unifiée porte le titre (posé par `router.ts::syncNav`), le contrôle segmenté
//       Session / Tout l'historique et la recherche. Rien de tout cela ne vit dans `#content`.
//   C — la table : Heure · Action · Piste · Destination · État, groupée par session, et par jour
//       au-dessus en mode historique.
//   D — l'inspecteur : résumé de session sans sélection, détail pour une entrée, agrégat pour
//       plusieurs. C'est LUI qui porte « Annuler », avec le menu contextuel — pas la ligne. Une
//       colonne de boutons sur un historique long coûte de la largeur en permanence pour un usage
//       rare.
//
// FRÉQUENCE DES GESTIONNAIRES — aucun n'est appelé en rafale ici, et c'est vérifiable : le Journal
// n'écoute AUCUN événement backend (ni `queue:changed`, ni progression, ni watcher). Tout ce qui
// repeint part d'un geste utilisateur : clic (~1 Hz au maximum), frappe dans la recherche
// (débattue, ~7 Hz au clavier, et le filtre est purement local — aucun aller-retour IPC), touche
// de navigation. Le seul `innerHTML` dans un chemin répété est celui du CORPS de la table sur
// frappe de recherche ; les marques d'état d'une ligne (sélection, annulation en cours, annulée,
// échec) se posent par mutation de classe sur les nœuds existants, jamais par reconstruction.
//
// QUATRE FONCTIONS DE MARKUP SONT PUBLIQUES — `theadHtml`, `skeletonHtml`, `rowHtml`, `groupHtml` —
// et elles ne lisent PAS `jrnlState` : ce que la vue sait (sélection, statut, racine, repli) leur
// arrive en argument. C'est ce qui permet à `journal-table.stories.ts` d'EXÉCUTER le vrai rendu au
// lieu d'en recopier ~90 lignes qui divergeaient en silence (modèle : `library-verdict.stories.ts`
// et `libraryTableRowHtml`). L'état entre en un seul point, `liveGroupHtml`.
import type { JournalEntry } from "../shared/contracts";
import { listJournal, getSessionId, revertBatch, revealTrack, getSetting } from "./ipc";
import { requireEl, esc, plural } from "./dom";
import { isStaleViewRender, viewEpoch } from "./view-epoch";
import { humanizeError } from "./errors";
import { confirmAction, BATCH_CONFIRM_THRESHOLD } from "./confirm-modal";
import { emptyStateHtml, wireEmptyState } from "./empty-state";
import { openContextMenu } from "./context-menu";
import { mountBarSearch, mountBarSegmented, openAside, closeAside } from "./toolbar";
import { toast, copyToClipboard } from "./filing-toast";
import { bibState } from "./bibliotheque-view";
import { sessionLabel } from "./session-label";

// ---------------------------------------------------------------------------
// État de l'écran
// ---------------------------------------------------------------------------

type JrnlMode = "session" | "all";

/** Ce que le front sait de l'issue d'une annulation. ABSENT (`null`) = « Appliqué ».
 *
 *  Deux sources, et l'ordre compte — il est écrit une seule fois, dans `statusOf`. « Annulé » vient
 *  de la DONNÉE : `JournalEntry.undone` traverse l'IPC depuis le 2026-08-19, donc l'état survit au
 *  rechargement et au retour sur l'écran. « Annulation… » et « Échec » restent purement locaux : ils
 *  décrivent une tentative de CETTE vue, que la base n'enregistre pas (un revert échoué laisse ses
 *  lignes telles quelles). */
export type JrnlStatus = "pending" | "reverted" | "failed";

const jrnlState: {
  mode: JrnlMode;
  entries: JournalEntry[];
  /** Index `batch_id` → entrée, reconstruit au SEUL point d'écriture d'`entries` (`loadAndPaint`).
   *  Quatre chemins cherchaient l'entrée par balayage linéaire — inspecteur, menu contextuel,
   *  double-clic, purge de sélection après une frappe — sur une liste qui monte à 500. */
  byId: Map<string, JournalEntry>;
  q: string;
  /** Clés de groupe repliées. Survit aux rendus, pas aux changements d'écran. */
  collapsed: Set<string>;
  /** Sélection, par `batch_id` — jamais par index : le filtre et le mode réordonnent la liste. */
  selection: Set<string>;
  anchor: string | null;
  status: Map<string, JrnlStatus>;
  /** Motif d'un échec d'annulation, montré dans l'inspecteur (spec, « Annulation échouée »). */
  failReason: Map<string, string>;
  /** Racine de bibliothèque, pour rendre une destination RELATIVE et non un chemin absolu. */
  root: string | null;
} = {
  mode: "session",
  entries: [],
  byId: new Map(),
  q: "",
  collapsed: new Set(),
  selection: new Set(),
  anchor: null,
  status: new Map(),
  failReason: new Map(),
  root: null,
};

// ---------------------------------------------------------------------------
// Helpers purs
// ---------------------------------------------------------------------------

function basenameNoExt(p: string | null): string {
  if (!p) return "";
  const i = Math.max(p.lastIndexOf("/"), p.lastIndexOf("\\"));
  const seg = i >= 0 ? p.slice(i + 1) : p;
  const dot = seg.lastIndexOf(".");
  return dot > 0 ? seg.slice(0, dot) : seg;
}

function extOf(p: string | null): string {
  if (!p) return "";
  const seg = p.slice(Math.max(p.lastIndexOf("/"), p.lastIndexOf("\\")) + 1);
  const dot = seg.lastIndexOf(".");
  return dot > 0 ? seg.slice(dot + 1).toUpperCase() : "";
}

/** Chemin de destination RELATIF à la racine de bibliothèque, quand elle est connue et qu'elle
 *  préfixe bien le chemin. Repli : les deux derniers segments, jamais un chemin absolu — une
 *  colonne de table n'est pas l'endroit où lire `C:\Users\…`.
 *
 *  La comparaison est insensible à la casse et normalise les séparateurs : la racine est saisie par
 *  l'utilisateur (sélecteur de dossier Windows) et le chemin vient de la base, les deux peuvent
 *  diverger sur `\` / `/` sans désigner autre chose. */
function relDest(p: string | null, root: string | null): string {
  if (!p) return "";
  const norm = (s: string) => s.replace(/\\/g, "/").replace(/\/+$/, "");
  const path = norm(p);
  if (root) {
    const r = norm(root);
    if (r && path.toLowerCase().startsWith(r.toLowerCase() + "/")) return path.slice(r.length + 1);
  }
  const parts = path.split("/").filter(Boolean);
  return parts.length >= 2 ? parts.slice(-2).join("/") : parts.join("/");
}

/** `actions.ts` est écrit par le DEFAULT SQLite `datetime('now')` (`db.rs`, migration v1), donc en
 *  **UTC**, au format `YYYY-MM-DD HH:MM:SS`. Sans le `T` et le `Z` explicites, V8 lit cette forme
 *  comme une heure LOCALE : l'écran afficherait l'heure UTC en la présentant comme locale — deux
 *  heures d'écart en France l'été, sans rien qui le signale. */
function parseTs(ts: string): Date | null {
  const iso = /^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}$/.test(ts) ? `${ts.replace(" ", "T")}Z` : ts;
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? null : d;
}

// Les trois formateurs sont construits UNE fois, au chargement du module. `Intl.DateTimeFormat`
// résout et compile sa locale à la construction, et `d.toLocaleTimeString("fr-FR", opts)` en
// refabrique un À CHAQUE APPEL — deux par ligne de table pour la seule heure, plus un par en-tête de
// jour. Les options sont exactement celles des trois `toLocale*` remplacées (hour+minute ;
// dateStyle+timeStyle ; weekday+day+month+year), et aucune n'entre dans le jeu « requis » qui
// déclencherait des valeurs par défaut différentes entre les deux formes : même sortie.
const TIME_FMT = new Intl.DateTimeFormat("fr-FR", { hour: "2-digit", minute: "2-digit" });
const STAMP_FMT = new Intl.DateTimeFormat("fr-FR", { dateStyle: "long", timeStyle: "medium" });
const DAY_FMT = new Intl.DateTimeFormat("fr-FR", {
  weekday: "long",
  day: "numeric",
  month: "long",
  year: "numeric",
});

function fmtTime(ts: string): string {
  const d = parseTs(ts);
  return d ? TIME_FMT.format(d) : ts;
}

/** `jj/mm` — la date courte qui accompagne une heure quand la plage traverse des jours. */
function fmtDay(ts: string): string {
  const d = parseTs(ts);
  if (!d) return "";
  return `${String(d.getDate()).padStart(2, "0")}/${String(d.getMonth() + 1).padStart(2, "0")}`;
}

function fmtStamp(ts: string): string {
  const d = parseTs(ts);
  return d ? STAMP_FMT.format(d) : ts;
}

/** Clé de jour LOCALE (et non la date de `ts`, qui est en UTC) : un rangement de 01 h 30 le
 *  20 août appartient au 20 août pour celui qui l'a fait, pas au 19 comme le dirait UTC. */
function dayKey(ts: string): string {
  const d = parseTs(ts);
  if (!d) return "?";
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

function dayLabel(ts: string): string {
  const d = parseTs(ts);
  if (!d) return "Date inconnue";
  return DAY_FMT.format(d);
}

// `sessionStart` / `sessionLabel` vivaient ici — partagés avec Rekordbox depuis le 2026-09-08
// (`session-label.ts`, module pur) : une session porte un seul nom dans toute l'app.

/** Vocabulaire d'action de la spec.
 *
 *  ⚠️ ÉCART SPEC ↔ RÉEL. La spec nomme cinq libellés — Rangé · Écarté · Restauré · Purgé · Tags
 *  appliqués — pour QUATRE `kind` au contrat (`shared/contracts.ts`, miroir d'`actions.rs`) :
 *  convert · move · trash · reject. « Tags appliqués » ne peut pas apparaître : `list_journal`
 *  exclut explicitement `type='tag_edit'` côté SQL. « Restauré » non plus : une restauration
 *  n'écrit aucune ligne de journal à elle. Les deux libellés sont donc laissés hors de cette
 *  table plutôt que d'être posés sur un `kind` qui ne les porte pas. */
function actionLabel(kind: JournalEntry["kind"]): string {
  switch (kind) {
    case "convert":
    case "move":
      return "Rangé";
    case "trash":
      return "Purgé";
    case "reject":
      return "Écarté";
  }
}

/** Ce qu'un statut vaut à l'écran : le mot de la cellule État, et la classe de la ligne.
 *
 *  UNE table, parce qu'il y en avait deux et demie — un `switch` de libellés, une chaîne de
 *  ternaires dans `rowHtml`, trois `classList.toggle` dans `paintRowStatus` — plus une quatrième
 *  copie dans `journal-table.stories.ts`, qui avait déjà DÉDUIT cette forme `Record<statut,
 *  {label, cls}>` : la story était la preuve que le besoin existait ici.
 *
 *  « Appliqué » n'y figure pas et n'a pas de classe : c'est l'ABSENCE de statut (`statusOf` → null),
 *  l'état de toute entrée que le backend vient de rendre. */
const ROW_STATE: Record<JrnlStatus, { label: string; cls: string }> = {
  pending: { label: "Annulation…", cls: "jrnl-row--pending" },
  reverted: { label: "Annulé", cls: "jrnl-row--reverted" },
  failed: { label: "Échec", cls: "jrnl-row--failed" },
};

/** L'état d'une entrée, en UN point. Deux sources et l'ordre compte : la carte locale d'abord — elle
 *  décrit une tentative de CETTE vue, que la base n'enregistre pas — puis la donnée, `undone`, qui
 *  fait survivre « Annulé » au rechargement comme au changement de mode.
 *
 *  Remplace un semis : `loadAndPaint` recopiait `e.undone` dans la carte à chaque lecture, et les
 *  cinq lecteurs se partageaient les deux sources sans le même ordre — `revertible` lisait la donnée
 *  d'abord, les quatre autres la carte seule. Le semis avait un effet de bord que ce `??` n'a plus :
 *  il ÉCRASAIT un « Échec » local dès que la base rendait le lot annulé. Le cas exige qu'un
 *  `revert_batch` ait jeté APRÈS avoir tout annulé, puis un changement de mode ; l'entrée s'y lit
 *  désormais « Échec » et reste rejouable — ce qu'un échec doit dire. */
function statusOf(e: JournalEntry): JrnlStatus | null {
  return jrnlState.status.get(e.batch_id) ?? (e.undone ? "reverted" : null);
}

/** Prend le STATUT et non l'entrée : `rowHtml` le tient de son argument (c'est ce qui le rend
 *  exécutable hors de la vue), les autres appelants le tirent de `statusOf(e)`. */
function statusLabel(s: JrnlStatus | null): string {
  return s ? ROW_STATE[s].label : "Appliqué";
}

/** Une entrée déjà annulée (ou en cours) ne se ré-annule pas. Tout ce qui décide d'un désarmement
 *  passe par ici — le bouton « Annuler cette action » de l'inspecteur, le `(N)` de « Annuler la
 *  sélection », le menu contextuel et la boucle de `revertSelection` — donc brancher CE point sur
 *  `statusOf` les branche tous les quatre sur la même lecture que la table.
 *
 *  Un lot PARTIELLEMENT annulé rend `undone:false` côté Rust (`MIN(undone)`) : il reste annulable,
 *  ce qui est exactement ce qu'il faut pour rejouer un revert interrompu. */
function revertible(e: JournalEntry): boolean {
  const s = statusOf(e);
  return s !== "reverted" && s !== "pending";
}

/** Nom affiché d'une piste.
 *
 *  ⚠️ ÉCART SPEC ↔ RÉEL. La spec demande « Artiste — titre, repli sur le nom de fichier » ;
 *  `JournalEntry` ne porte NI artiste NI titre — seulement `from_path`/`to_path`
 *  (`shared/contracts.ts:290`). Le nom de fichier rangé est donc tout ce que cet écran peut dire
 *  honnêtement. Comme le rangement écrit le fichier depuis le gabarit de nommage, il vaut le plus
 *  souvent « Artiste - Titre » — mais c'est le nom du fichier, pas le champ, et rien ici ne le
 *  garantit. Afficher les deux vrais champs demanderait de les joindre côté Rust. */
function trackName(e: JournalEntry): string {
  const n = basenameNoExt(e.to_path ?? e.from_path);
  return n || "—";
}

// ---------------------------------------------------------------------------
// Filtre et regroupement
// ---------------------------------------------------------------------------

function matchesQuery(e: JournalEntry, q: string): boolean {
  if (!q) return true;
  const hay = `${trackName(e)}\n${relDest(e.to_path, jrnlState.root)}`.toLowerCase();
  return hay.includes(q);
}

export interface JrnlGroup {
  key: string;
  label: string;
  entries: JournalEntry[];
  /** Sous-groupes (mode historique : jour → sessions). Vide en mode session. */
  children: JrnlGroup[];
}

/** Groupes dans l'ordre d'affichage. Mode session : un niveau (la session). Mode historique : deux
 *  (jour, puis session) — la spec demande le jour comme niveau supplémentaire AU-DESSUS, parce que
 *  c'est par là qu'on cherche dans un historique long.
 *
 *  Pas de tableau `order` en parallèle des `Map` : une `Map` itère dans l'ordre d'INSERTION, donc
 *  `[...map.values()]` rend déjà les groupes dans l'ordre où la première entrée de chacun est
 *  arrivée. La fabrique passée à `push` porte aussi les libellés, qui ne sont donc calculés qu'à la
 *  CRÉATION du groupe — `dayLabel` (un `Intl.format`) partait une fois par entrée pour un résultat
 *  constant sur toute une journée. */
function buildGroups(entries: JournalEntry[]): JrnlGroup[] {
  const push = (map: Map<string, JrnlGroup>, key: string, make: () => JrnlGroup): JrnlGroup => {
    let g = map.get(key);
    if (!g) {
      g = make();
      map.set(key, g);
    }
    return g;
  };

  if (jrnlState.mode === "session") {
    const map = new Map<string, JrnlGroup>();
    for (const e of entries) {
      const key = `s:${e.session_id ?? "none"}`;
      push(map, key, () => ({
        key,
        label: sessionLabel(e.session_id, true),
        entries: [],
        children: [],
      })).entries.push(e);
    }
    return [...map.values()];
  }

  const days = new Map<string, JrnlGroup>();
  const sessions = new Map<string, JrnlGroup>();
  for (const e of entries) {
    const dk = dayKey(e.ts);
    const dKey = `d:${dk}`;
    const day = push(days, dKey, () => ({ key: dKey, label: dayLabel(e.ts), entries: [], children: [] }));
    day.entries.push(e);
    const sKey = `s:${dk}:${e.session_id ?? "none"}`;
    // Le rattachement au jour vit DANS la fabrique : elle ne tourne qu'à la création, donc le
    // sous-groupe n'entre qu'une fois dans `day.children`, sans test d'appartenance.
    const sess = push(sessions, sKey, () => {
      const s: JrnlGroup = {
        key: sKey,
        label: sessionLabel(e.session_id, false),
        entries: [],
        children: [],
      };
      day.children.push(s);
      return s;
    });
    sess.entries.push(e);
  }
  return [...days.values()];
}

/** Les `batch_id` VISIBLES, dans l'ordre affiché — un groupe replié n'en fournit aucun.
 *
 *  C'est cette liste que le clavier parcourt par INDEX (DESIGN.md § 9, couche 2), jamais les nœuds
 *  du DOM : marcher le DOM s'arrêterait au bord de ce qui se trouve rendu, et se tromperait de
 *  voisin dès qu'un groupe est replié. Même leçon que `stepBibSelection`. */
function visibleOrder(groups: JrnlGroup[]): string[] {
  const out: string[] = [];
  for (const g of groups) {
    if (jrnlState.collapsed.has(g.key)) continue;
    if (g.children.length) {
      for (const c of g.children) {
        if (jrnlState.collapsed.has(c.key)) continue;
        for (const e of c.entries) out.push(e.batch_id);
      }
    } else {
      for (const e of g.entries) out.push(e.batch_id);
    }
  }
  return out;
}

function currentGroups(): JrnlGroup[] {
  const q = jrnlState.q.trim().toLowerCase();
  return buildGroups(jrnlState.entries.filter((e) => matchesQuery(e, q)));
}

/** L'ordre visible pour l'état COURANT — filtre et replis compris. Les quatre appelants (clic ⇧,
 *  menu contextuel, flèches, ⌘A) en avaient la même paire d'appels écrite à la main. */
function visibleIds(): string[] {
  return visibleOrder(currentGroups());
}

function entryById(id: string): JournalEntry | undefined {
  return jrnlState.byId.get(id);
}

// ---------------------------------------------------------------------------
// Markup
// ---------------------------------------------------------------------------

const COLS: readonly { cls: string; label: string }[] = [
  { cls: "jrnl-c-time", label: "Heure" },
  { cls: "jrnl-c-act", label: "Action" },
  { cls: "jrnl-c-track", label: "Piste" },
  { cls: "jrnl-c-dest", label: "Destination" },
  { cls: "jrnl-c-state", label: "État" },
];

/** Ligne d'en-tête, collante en haut de la zone C — même grammaire que `.sift-lib-thead` de
 *  Bibliothèque, classe propre : le clic droit sur `.sift-lib-thead` ouvre les réglages de COLONNES
 *  de la Bibliothèque (`sift-live.ts`), et le Journal n'a ni tri ni colonnes redimensionnables. */
export function theadHtml(): string {
  return (
    `<div class="jrnl-thead" role="row">` +
    COLS.map((c) => `<span class="jrnl-c ${c.cls}" role="columnheader">${c.label}</span>`).join("") +
    `</div>`
  );
}

/** Tout ce que `rowHtml` a besoin de savoir de la VUE — et rien de plus. La ligne ne lit donc pas
 *  `jrnlState` : elle est peignable depuis une story comme depuis l'app. */
export interface JrnlRowView {
  selected: boolean;
  status: JrnlStatus | null;
  /** Racine de bibliothèque, pour rendre la destination relative (`relDest`). */
  root: string | null;
}

export function rowHtml(e: JournalEntry, view: JrnlRowView): string {
  const id = esc(e.batch_id);
  // Une seule fois : l'heure sert à la cellule ET à l'`aria-label`, et `fmtTime` traverse un
  // `parseTs` + un formatage par appel.
  const time = fmtTime(e.ts);
  const act = actionLabel(e.kind);
  const name = trackName(e);
  const dest = relDest(e.to_path, view.root);
  const state = statusLabel(view.status);
  const cls =
    "lr jrnl-row" + (view.selected ? " sel" : "") + (view.status ? ` ${ROW_STATE[view.status].cls}` : "");
  const count = e.track_count > 1 ? ` (${e.track_count} morceaux)` : "";
  const label = `${time}, ${act}${count}, ${name}, ${dest || "sans destination"}, ${state}`;
  return (
    `<div class="${cls}" data-jrow="${id}" tabindex="0" role="option" aria-selected="${view.selected}" ` +
    `aria-label="${esc(label)}">` +
    `<span class="jrnl-c jrnl-c-time">${esc(time)}</span>` +
    `<span class="jrnl-c jrnl-c-act">${esc(act)}${e.track_count > 1 ? `<span class="jrnl-batch">×${e.track_count}</span>` : ""}</span>` +
    `<span class="jrnl-c jrnl-c-track">${esc(name)}</span>` +
    // `<bdi>` n'est pas décoratif : la cellule est en `direction:rtl` pour tronquer par la GAUCHE,
    // et sans isolation l'algorithme bidi remonte en fin de ligne tout segment initial neutre ou
    // faible — un dossier nommé « (2002) The Universal Sky » s'affichait
    // « The Universal Sky … (2002) ». Un chemin peint dans le mauvais ordre, dans une app dont le
    // métier est le rangement de fichiers. Mesuré dans la vraie fenêtre le 2026-08-19 : le
    // `textContent` était juste, seul le rendu mentait.
    `<span class="jrnl-c jrnl-c-dest" title="${esc(e.to_path ?? "")}"><bdi>${esc(dest || "—")}</bdi></span>` +
    `<span class="jrnl-c jrnl-c-state">${esc(state)}</span>` +
    `</div>`
  );
}

/** En-tête repliable + corps. `open` et `body` sont des ARGUMENTS : le repli est un état de vue
 *  (`jrnlState.collapsed`) et le corps peut être des lignes comme des sous-groupes — les décider ici
 *  rendrait la fonction dépendante de l'écran. Corps VIDÉ quand le groupe est replié, pas caché. */
export function groupHtml(g: JrnlGroup, level: 1 | 2, open: boolean, body: string): string {
  return (
    `<div class="jrnl-group jrnl-group--l${level}">` +
    `<button type="button" class="jrnl-group-hd" data-jgroup="${esc(g.key)}" aria-expanded="${open}">` +
    `<i class="ti ti-chevron-right jrnl-group-chev" aria-hidden="true"></i>` +
    `<span class="jrnl-group-label">${esc(g.label)}</span>` +
    `<span class="jrnl-group-count">${esc(plural(g.entries.length, "action"))}</span>` +
    `</button>` +
    `<div class="jrnl-group-body">${body}</div>` +
    `</div>`
  );
}

/** Squelette DANS la structure finale (DESIGN.md § 8) : mêmes colonnes, mêmes hauteurs de ligne,
 *  pour que l'arrivée des données ne déplace rien. */
export function skeletonHtml(): string {
  const cell = (c: string) => `<span class="jrnl-c ${c}"><span class="sift-skel"></span></span>`;
  const row = `<div class="lr jrnl-row jrnl-row--skel" aria-hidden="true">${COLS.map((c) => cell(c.cls)).join("")}</div>`;
  return `<div class="jrnl-body">${row.repeat(6)}</div>`;
}

/** LE point où `jrnlState` entre dans le markup, et le seul : sous lui, `groupHtml` et `rowHtml` ne
 *  lisent que leurs arguments. */
function liveGroupHtml(g: JrnlGroup, level: 1 | 2): string {
  const open = !jrnlState.collapsed.has(g.key);
  const body = !open
    ? ""
    : g.children.length
      ? g.children.map((c) => liveGroupHtml(c, 2)).join("")
      : g.entries
          .map((e) =>
            rowHtml(e, {
              selected: jrnlState.selection.has(e.batch_id),
              status: statusOf(e),
              root: jrnlState.root,
            }),
          )
          .join("");
  return groupHtml(g, level, open, body);
}

// ---------------------------------------------------------------------------
// Rendu
// ---------------------------------------------------------------------------

/** Peint le Journal en rattrapant l'échec, pour les appelants qui ne peuvent pas `await`.
 *
 *  `reset` distingue les deux entrées : le routeur ouvre l'écran (`true` — état d'ouverture remis à
 *  neuf par `renderJournal`), le contrôle segmenté et le bouton « Réessayer » rechargent seulement
 *  (`false`). Le retry NE réinitialise donc PAS le mode, et c'est la sémantique d'avant : la HOF à
 *  deux paramètres qu'il remplace se rappelait avec la même fonction de rendu, mais `renderJournal`
 *  avait déjà posé son état AVANT le jet — le rejouer était sans effet.
 *
 *  Impasse A18 (issue #15) : les `void render…()` laissaient partir la promesse sans `.catch`,
 *  alors que le routeur a DÉJÀ vidé `#content` avant de déléguer. Un rejet ne donnait donc même pas
 *  un état vide : un écran nu, et une unhandled rejection en console que personne ne lit.
 *
 *  L'erreur passe avant tout le reste (DESIGN.md § 8) : sur une lecture échouée, l'écran n'affirme
 *  RIEN du contenu — pas de « rien dans cette session », qui serait un fait non mesuré. */
export function paintJournal(reset = false): void {
  // Jeton pris À L'APPEL, pas dans le `catch` : c'est l'écran pour lequel ce rendu a été demandé
  // qu'on veut comparer, et le `catch` s'exécute après l'attente (issue #42).
  const token = viewEpoch();
  void (reset ? renderJournal() : loadAndPaint()).catch((e: unknown) => {
    if (isStaleViewRender(token)) return;
    const display = humanizeError(
      e,
      "Impossible de lire le journal. Vérifie la connexion à la base et réessaie.",
      `renderJournal(${jrnlState.mode})`,
    );
    // `requireEl` lèverait à son tour si `#content` manquait — dans un `catch`, on ne peut pas se
    // permettre un second échec.
    const content = document.getElementById("content");
    if (!content) return;
    content.innerHTML =
      `<div class="jrnl-wrap">` +
      `<div class="sift-ui-card-soft sift-ui-card-soft-pad jrnl-error">` +
      esc(display) +
      `<div class="jrnl-error-actions"><button type="button" data-jact="retry">Réessayer</button></div>` +
      `</div></div>`;
    content
      .querySelector<HTMLButtonElement>('[data-jact="retry"]')
      ?.addEventListener("click", () => paintJournal());
  });
}

/** Remet la vue à son état d'ouverture — mode session, sans filtre ni sélection héritée d'une visite
 *  précédente. Passe par `paintJournal(true)` : le routeur ne peut pas `await`. */
export async function renderJournal(): Promise<void> {
  jrnlState.mode = "session";
  jrnlState.q = "";
  jrnlState.selection.clear();
  jrnlState.anchor = null;
  jrnlState.status.clear();
  jrnlState.failReason.clear();
  await loadAndPaint();
}

/** Change de mode sans perdre l'écran : la barre est déjà montée, seul le corps se recharge. */
function switchMode(mode: JrnlMode): void {
  if (mode === jrnlState.mode) return;
  jrnlState.mode = mode;
  jrnlState.selection.clear();
  jrnlState.anchor = null;
  paintJournal();
}

async function loadAndPaint(): Promise<void> {
  const content = requireEl<HTMLElement>("#content", "renderJournal");
  const token = viewEpoch();
  // Le SEUL point qui lève le drapeau, et il couvre les deux issues : le rendu abouti pose
  // `.jrnl-wrap` par `paintTable`, la lecture échouée par le `catch` de `paintJournal`. Voir
  // `journalOnScreen`.
  journalPainted = true;

  // La barre AVANT la lecture : ses deux contrôles ne dépendent pas des données, et une lecture qui
  // échoue doit laisser le mode changeable — sinon l'écran d'erreur est une impasse.
  mountBar();

  // Squelette seulement quand rien de valide n'est affiché (DESIGN.md § 8 : ne jamais vider
  // l'écran pour recharger). Même garde que `renderBiblioLive`, même repère : un nœud que seul le
  // rendu abouti pose.
  if (!content.querySelector(".jrnl-body")) {
    content.innerHTML = `<div class="jrnl-wrap">${theadHtml()}${skeletonHtml()}</div>`;
  }

  const all = jrnlState.mode === "all";
  // `getSetting` rejoint le même aller-retour : la racine sert à rendre une destination RELATIVE,
  // et la relire à chaque rendu la garde juste après un changement dans Réglages.
  const [entries, root] = await Promise.all([
    all ? listJournal(500) : getSessionId().then((sid) => listJournal(50, sid)),
    getSetting("library_root").catch((e: unknown) => {
      // Un réglage illisible ne doit pas emporter la lecture du journal : la destination se replie
      // alors sur ses deux derniers segments. C'est une dégradation NOMMÉE, pas un silence.
      console.error("[journal] getSetting(library_root) failed", e);
      return null;
    }),
  ]);
  // L'écran a pu changer pendant l'attente (issue #42) : ne rien peindre, et surtout ne pas semer
  // `jrnlState` depuis un rendu que plus personne ne regarde.
  if (isStaleViewRender(token)) return;

  jrnlState.entries = entries;
  jrnlState.byId = new Map(entries.map((e) => [e.batch_id, e]));
  jrnlState.root = root;
  // Aucun semis de statuts : `statusOf` lit `undone` directement sur l'entrée. Une sélection qui
  // désigne une entrée disparue, elle, ne pointe plus rien — et l'index sert de test d'existence.
  for (const id of [...jrnlState.selection]) if (!jrnlState.byId.has(id)) jrnlState.selection.delete(id);
  if (jrnlState.anchor && !jrnlState.byId.has(jrnlState.anchor)) jrnlState.anchor = null;

  paintTable(content);
  installJournalHandlers();
  // Le compte à côté du titre, comme Revue, Rangés et Écartés (2026-09-08, déclinaison #24) : ce
  // que la table MONTRE — le mode et le filtre compris —, jamais un total global. Le résumé de
  // l'inspecteur garde le sien : l'un situe l'écran, l'autre décrit la sélection.
  const countEl = document.getElementById("sift-tb-count");
  if (countEl) {
    const q = jrnlState.q.trim().toLowerCase();
    const n = jrnlState.entries.filter((e) => matchesQuery(e, q)).length;
    countEl.textContent = n ? plural(n, "action") : "";
  }
  // Rien du tout : l'écran est une impasse assumée, et un inspecteur qui annonce « 0 action » à
  // côté n'ajoute rien. Même geste que `renderBiblioLive`, qui ne monte pas sa zone D sur une
  // bibliothèque vide.
  if (jrnlState.entries.length === 0) closeAside();
  else paintAside();
}

/** UN seul wrapper pour tout ce que la vue pose dans `#content` (règle `CLAUDE.md` § Front) :
 *  retiré et recréé en un point unique, ici. */
function paintTable(content: HTMLElement): void {
  resetSelectionMirror();

  if (jrnlState.entries.length === 0) {
    // Impasse assumée : rien à filtrer, donc pas d'en-tête de colonnes à garder à l'écran.
    const empty =
      jrnlState.mode === "all"
        ? {
            title: "Aucune action enregistrée",
            note: "L'historique complet des rangements, écarts et purges apparaîtra ici, prêt à être annulé.",
          }
        : {
            title: "Rien dans cette session",
            note: "Les actions de cette session apparaissent ici au fur et à mesure. L'historique complet reste accessible depuis la barre.",
          };
    content.innerHTML = `<div class="jrnl-wrap">${emptyStateHtml({ ...empty, backToRevue: true })}</div>`;
    wireEmptyState(content);
    return;
  }

  content.innerHTML = `<div class="jrnl-wrap">${theadHtml()}${bodyHtml(currentGroups())}</div>`;
}

/** Le corps, groupes ou « aucun résultat ». Un seul endroit le construit : `paintTable` (rendu
 *  complet) et `repaintBody` (frappe de recherche, repli de groupe) en avaient deux copies, donc
 *  deux façons de diverger.
 *
 *  Filtre qui ne rend rien : l'en-tête de colonnes RESTE, et la recherche avec lui — on ne retire
 *  pas les commandes qui permettent de défaire le filtre (même règle que la table Bibliothèque). */
function bodyHtml(groups: JrnlGroup[]): string {
  if (!groups.length) {
    return `<div class="jrnl-body"><div class="jrnl-noresult">Aucune action ne correspond à « ${esc(jrnlState.q)} ».</div></div>`;
  }
  return `<div class="jrnl-body">${groups.map((g) => liveGroupHtml(g, 1)).join("")}</div>`;
}

/** Repeint le CORPS seul — pour la frappe de recherche et le repli d'un groupe, qui ne relisent
 *  aucune donnée. Le reste de l'écran (en-tête de colonnes, barre) ne bouge pas. */
function repaintBody(): void {
  const content = document.getElementById("content");
  const wrap = content?.querySelector<HTMLElement>(".jrnl-wrap");
  if (!content || !wrap) return;
  const old = wrap.querySelector(".jrnl-body");
  if (!old) {
    paintTable(content);
    return;
  }
  old.outerHTML = bodyHtml(currentGroups());
  resetSelectionMirror();
}

function mountBar(): void {
  mountBarSegmented({
    id: "sift-jrnl-seg",
    ariaLabel: "Portée du journal",
    options: [
      { id: "session", label: "Session" },
      { id: "all", label: "Tout l'historique" },
    ],
    active: jrnlState.mode,
    onPick: (id) => switchMode(id === "all" ? "all" : "session"),
  });
  mountBarSearch({
    placeholder: "Rechercher…",
    ariaLabel: "Filtrer le journal par nom de fichier ou destination",
    value: jrnlState.q,
    onInput: (value) => {
      jrnlState.q = value;
      // Anti-rebond court : le filtre est LOCAL (aucun IPC), il n'y a rien à protéger d'autre que
      // le coût de reconstruire le corps entre deux touches.
      clearTimeout(searchTimer);
      searchTimer = window.setTimeout(() => {
        // Le filtre RETIRE de la sélection ce qu'il cache. Mesuré dans la vraie fenêtre le
        // 2026-08-19 : après un ⌘A puis une frappe, la table montrait une ligne et l'inspecteur
        // proposait toujours « Annuler la sélection (5) » — quatre actions qu'on ne voit plus.
        // Un repli de groupe, lui, ne purge rien : la ligne est cachée mais son groupe est là,
        // avec son compte, et rien ne prétend le contraire.
        const q = jrnlState.q.trim().toLowerCase();
        for (const id of [...jrnlState.selection]) {
          const e = entryById(id);
          if (!e || !matchesQuery(e, q)) jrnlState.selection.delete(id);
        }
        if (jrnlState.anchor && !jrnlState.selection.has(jrnlState.anchor)) jrnlState.anchor = null;
        repaintBody();
        paintAside();
      }, 120);
    },
  });
}

let searchTimer: number | undefined;

// ---------------------------------------------------------------------------
// Sélection
// ---------------------------------------------------------------------------

function bodyEl(): HTMLElement | null {
  return document.querySelector<HTMLElement>("#content .jrnl-body");
}

function rowEl(id: string): HTMLElement | null {
  return bodyEl()?.querySelector<HTMLElement>(`.jrnl-row[data-jrow="${CSS.escape(id)}"]`) ?? null;
}

/** Remplace la sélection par la plage `[a,b]` de `ids`, bornes comprises et dans les deux sens.
 *  Écrite deux fois à l'identique — clic ⇧ et ⇧+flèche. */
function selectRange(ids: readonly string[], a: number, b: number): void {
  jrnlState.selection.clear();
  for (let i = Math.min(a, b); i <= Math.max(a, b); i++) jrnlState.selection.add(ids[i]);
}

/** Applique un clic de ligne à la convention système : clic remplace, ⇧ étend depuis l'ancre,
 *  ⌘/Ctrl bascule. Calqué sur `applyRowClick` (Bibliothèque) — un seul geste à apprendre pour les
 *  deux tables.
 *
 *  L'ordre visible se calcule DANS la branche ⇧ : c'est la seule qui s'en sert, et le reconstruire
 *  demande de refiltrer et de regrouper toutes les entrées — pour un clic simple, la dépense était
 *  entière et le résultat jeté. */
function applyJrnlClick(id: string, mods: { shift: boolean; meta: boolean }): void {
  if (mods.shift && jrnlState.anchor != null) {
    const ordered = visibleIds();
    const a = ordered.indexOf(jrnlState.anchor);
    const b = ordered.indexOf(id);
    if (a >= 0 && b >= 0) {
      selectRange(ordered, a, b);
      return;
    }
  }
  if (mods.meta) {
    if (jrnlState.selection.has(id)) jrnlState.selection.delete(id);
    else jrnlState.selection.add(id);
    jrnlState.anchor = id;
    return;
  }
  jrnlState.selection.clear();
  jrnlState.selection.add(id);
  jrnlState.anchor = id;
}

/** Ce que les lignes MONTÉES portent réellement comme marque de sélection — le miroir dont
 *  `paintSelection` déduit les seules lignes à toucher. */
let selPainted = new Set<string>();

/** Toute reconstruction du corps repeint les marques depuis `jrnlState.selection` (c'est `rowHtml`
 *  qui les pose) : le miroir doit repartir de là, sinon il reste en AVANCE sur le DOM et
 *  `paintSelection` saute une ligne dont la marque vient d'être refaite. Le cas se produit dès que
 *  la sélection change sans repeinte — la purge de la frappe de recherche — puis qu'un ⌘A la
 *  ré-ajoute : la ligne est dans l'ancien miroir ET dans la nouvelle sélection, donc hors de la
 *  différence symétrique, et elle resterait sélectionnée en état sans l'être à l'écran. */
function resetSelectionMirror(): void {
  selPainted = new Set(jrnlState.selection);
}

/** Repeint la marque de sélection sur les lignes MONTÉES, sans reconstruire la table : le menu
 *  contextuel se ferme au premier défilement, et un rebuild en émettrait un.
 *
 *  Ne touche QUE la différence symétrique avec l'état déjà peint, et cherche sous `.jrnl-body` :
 *  la forme précédente balayait `document` en entier et réécrivait deux attributs sur les 500
 *  lignes à chaque flèche du clavier, pour en changer deux. */
function paintSelection(): void {
  const body = bodyEl();
  if (!body) return;
  const touched: string[] = [];
  for (const id of selPainted) if (!jrnlState.selection.has(id)) touched.push(id);
  for (const id of jrnlState.selection) if (!selPainted.has(id)) touched.push(id);
  for (const id of touched) {
    const n = body.querySelector<HTMLElement>(`.jrnl-row[data-jrow="${CSS.escape(id)}"]`);
    if (!n) continue;
    const on = jrnlState.selection.has(id);
    n.classList.toggle("sel", on);
    n.setAttribute("aria-selected", on ? "true" : "false");
  }
  resetSelectionMirror();
}

/** La paire que tout changement de sélection doit poser : la marque sur les lignes, et l'inspecteur
 *  qui décrit ce qui est sélectionné. Quatre sites l'écrivaient à la suite. */
function syncSelectionUi(): void {
  paintSelection();
  paintAside();
}

/** Repeint l'état d'UNE ligne (cellule État + classe), par mutation. Aucune reconstruction : la
 *  table reste utilisable pendant qu'une annulation tourne (spec, « Annulation en cours »). */
function paintRowStatus(e: JournalEntry): void {
  const row = rowEl(e.batch_id);
  if (!row) return;
  const st = statusOf(e);
  for (const [key, v] of Object.entries(ROW_STATE)) row.classList.toggle(v.cls, key === st);
  const cell = row.querySelector<HTMLElement>(".jrnl-c-state");
  if (cell) cell.textContent = statusLabel(st);
  if (st === "reverted") {
    // SEULE la transition se colore ; l'état annulé, lui, reste neutre et permanent (DESIGN.md
    // § 8). La classe se retire à `animationend` — sinon une seconde annulation sur la même ligne
    // ne rejouerait rien. Le bloc `prefers-reduced-motion` du dépôt met la durée à ~0 au lieu de
    // supprimer l'animation, donc l'événement arrive dans tous les cas.
    row.classList.add("jrnl-row--flash");
    row.addEventListener("animationend", () => row.classList.remove("jrnl-row--flash"), { once: true });
  }
}

// ---------------------------------------------------------------------------
// Zone D — l'inspecteur
// ---------------------------------------------------------------------------

function rowsHtml(pairs: readonly [string, string][]): string {
  return (
    `<dl class="sift-sel-rows">` +
    pairs.map(([k, v]) => `<dt>${esc(k)}</dt><dd>${esc(v)}</dd>`).join("") +
    `</dl>`
  );
}

/** « combien par libellé d'action », dans l'ordre de première apparition. Les deux inspecteurs qui
 *  le montrent — le résumé sans sélection et l'agrégat multi-sélection — en avaient chacun leur
 *  copie de la boucle. */
function countByAction(entries: readonly JournalEntry[]): [string, string][] {
  const byAction = new Map<string, number>();
  for (const e of entries) {
    const l = actionLabel(e.kind);
    byAction.set(l, (byAction.get(l) ?? 0) + 1);
  }
  return [...byAction].map(([k, v]): [string, string] => [k, String(v)]);
}

function paintAside(): void {
  const host = openAside();
  if (!host) return;
  const ids = [...jrnlState.selection];
  if (ids.length === 0) {
    host.innerHTML = asideSummaryHtml();
    return;
  }
  if (ids.length === 1) {
    const e = entryById(ids[0]);
    host.innerHTML = e ? asideOneHtml(e) : asideSummaryHtml();
  } else {
    host.innerHTML = asideManyHtml(ids);
  }
  wireAside(host);
}

/** Aucune sélection — le résumé de ce que la table montre. Jamais un état vide : l'inspecteur
 *  porte le contexte de la zone C (DESIGN.md § 14). */
function asideSummaryHtml(): string {
  // `q` normalisé UNE fois, hors du filtre : `trim().toLowerCase()` par entrée allouait deux
  // chaînes par ligne pour un résultat constant.
  const q = jrnlState.q.trim().toLowerCase();
  const shown = jrnlState.entries.filter((e) => matchesQuery(e, q));
  const pairs = countByAction(shown);
  // Un compteur à zéro en permanence occupe la place d'une information sans en porter : les échecs
  // ne s'affichent que s'il y en a. Ils ne s'atténuent jamais pour autant (DESIGN.md § 4).
  const failed = shown.filter((e) => statusOf(e) === "failed").length;
  if (failed) pairs.push(["Échecs", String(failed)]);
  // Le backend rend du plus récent au plus ancien : les bornes de la plage sont les deux BOUTS de
  // la liste, pas une accumulation.
  const first = shown.length ? shown[0].ts : null;
  const last = shown.length ? shown[shown.length - 1].ts : null;
  if (first && last) {
    // Une heure seule ne situe rien quand la plage traverse des jours : en mode historique elle
    // dirait « 19:32 → 21:23 » pour trois semaines d'écart. La date rejoint donc l'heure là, et
    // seulement là — dans une session, tout est du même jour et la répéter serait du bruit.
    const at = (ts: string) => (jrnlState.mode === "all" ? `${fmtDay(ts)} ${fmtTime(ts)}` : fmtTime(ts));
    pairs.push(["Plage horaire", first === last ? at(first) : `${at(last)} → ${at(first)}`]);
  }
  const title = jrnlState.mode === "all" ? "Tout l'historique" : "Session courante";
  return (
    `<div class="col-h">${esc(title)}</div>` +
    `<div class="sift-sel-count">${esc(plural(shown.length, "action"))}</div>` +
    rowsHtml(pairs) +
    (failed
      ? `<div class="jrnl-insp-note">Sélectionne une ligne en échec pour lire son motif.</div>`
      : "")
  );
}

function asideOneHtml(e: JournalEntry): string {
  const fmt = extOf(e.to_path);
  // Pas de ligne « Piste » : le titre de l'inspecteur porte déjà le nom, et le répéter en paire
  // libellé/valeur le donnait deux fois à la suite.
  const pairs: [string, string][] = [["Horodatage", fmtStamp(e.ts)]];
  if (e.track_count > 1) pairs.push(["Morceaux du lot", String(e.track_count)]);
  if (fmt) pairs.push(["Format produit", fmt]);
  pairs.push(["État", statusLabel(statusOf(e))]);
  const reason = jrnlState.failReason.get(e.batch_id);
  const applicable = revertible(e);
  return (
    `<div class="col-h">${esc(actionLabel(e.kind))}</div>` +
    `<div class="sift-sel-count jrnl-insp-title">${esc(trackName(e))}</div>` +
    rowsHtml(pairs) +
    (reason ? `<div class="jrnl-insp-fail">${esc(reason)}</div>` : "") +
    pathBlock("Source", e.from_path) +
    pathBlock("Destination", e.to_path) +
    `<div class="jrnl-insp-actions">` +
    `<button type="button" data-jact="revert-sel"${applicable ? "" : " disabled"}>Annuler cette action</button>` +
    `</div>`
  );
}

function pathBlock(label: string, p: string | null): string {
  if (!p) return "";
  return `<div class="jrnl-insp-path"><div class="col-h">${esc(label)}</div><div class="jrnl-insp-pathval">${esc(p)}</div></div>`;
}

function asideManyHtml(ids: string[]): string {
  const picked = ids.map(entryById).filter((e): e is JournalEntry => !!e);
  const applicable = picked.filter(revertible);
  const pairs = countByAction(picked);
  pairs.push(["Morceaux concernés", String(picked.reduce((s, e) => s + e.track_count, 0))]);
  if (applicable.length !== picked.length) {
    pairs.push(["Déjà annulées", String(picked.length - applicable.length)]);
  }
  return (
    `<div class="col-h">Sélection</div>` +
    `<div class="sift-sel-count">${esc(plural(picked.length, "action"))}</div>` +
    rowsHtml(pairs) +
    `<div class="jrnl-insp-actions">` +
    `<button type="button" data-jact="revert-sel"${applicable.length ? "" : " disabled"}>` +
    `Annuler la sélection (${applicable.length})</button>` +
    `</div>`
  );
}

/** L'inspecteur est reconstruit à chaque changement de sélection : ses écouteurs partent avec ses
 *  nœuds, rien ne s'accumule. Fréquence : un clic utilisateur. */
function wireAside(host: HTMLElement): void {
  host
    .querySelector<HTMLButtonElement>('[data-jact="revert-sel"]')
    ?.addEventListener("click", () => void revertSelection());
}

// ---------------------------------------------------------------------------
// Annulation
// ---------------------------------------------------------------------------

/** La table de domaine reste ici — elle est PLUS précise que tout repli générique (`errors.ts`) —
 *  mais la plomberie part : `humanizeError` garantit le `console.error`, et lui passe l'objet
 *  d'erreur, donc la STACK repart en console au lieu du seul `message` que ce site en tirait. */
function humanRevertError(err: unknown): string {
  const raw = err instanceof Error ? err.message : String(err);
  let display = "Annulation impossible — réessaie.";
  if (raw.includes("destination occupied"))
    display = "Fichier déjà à l'emplacement d'origine — doublon probable (sync cloud ?).";
  else if (raw.includes("source gone"))
    display = "Fichier introuvable à destination — déplacé ou supprimé manuellement ?";
  else if (raw.includes("newer action")) display = "Action plus récente à annuler d'abord.";
  return humanizeError(err, display, "revert_batch");
}

/** Un cycle d'annulation est-il en cours ? Voir le garde de `revertSelection`. */
let revertRunning = false;

/** Annule les entrées sélectionnées encore annulables.
 *
 *  SÉQUENTIEL, et ce n'est pas un oubli : le backend est synchrone derrière un `Mutex<Connection>`
 *  non réentrant (`db::lock_conn`). Des `revert_batch` concurrents se sérialiseraient de toute
 *  façon, en bloquant tout le reste de l'IPC pendant ce temps.
 *
 *  La table NE SE RECHARGE PAS après coup, et ce n'est plus la même raison qu'avant. Le
 *  rechargement RENDRAIT désormais les bonnes lignes (`list_journal` ne filtre plus `undone=0`) ;
 *  il est simplement inutile — la mutation locale montre le résultat sans aller-retour, et sans
 *  faire sauter la position de défilement ni la sélection sous la main de l'utilisateur. */
async function revertSelection(): Promise<void> {
  // Un seul cycle d'annulation à la fois. Les entrées déjà en cours sont exclues par
  // `revertible`, mais rien n'empêcherait autrement de changer de sélection pendant la boucle et
  // d'en lancer une SECONDE en parallèle : deux séries d'`invoke` concurrentes sur une frontière
  // synchrone que le `Mutex<Connection>` sérialise de toute façon, en bloquant tout le reste de
  // l'IPC pendant ce temps. C'est le garde que l'ancien Journal obtenait en désarmant tous ses
  // boutons de ligne ; les boutons ayant quitté la ligne, le garde vit ici.
  if (revertRunning) return;
  const picked = [...jrnlState.selection]
    .map(entryById)
    .filter((e): e is JournalEntry => !!e && revertible(e));
  if (picked.length === 0) return;

  const tracks = picked.reduce((s, e) => s + e.track_count, 0);
  // Confirmation in-app armée et horodatée AU-DELÀ DU SEUIL (`confirm-modal.ts`), avec le nombre
  // exact dans le libellé. Jamais `window.confirm()` : un clic synthétique en a déjà traversé un.
  if (
    tracks > BATCH_CONFIRM_THRESHOLD &&
    !(await confirmAction(
      `Annuler ${plural(picked.length, "action")} du journal (${plural(tracks, "morceau", "morceaux")}) ?`,
      "Annuler ces actions",
    ))
  )
    return;

  revertRunning = true;
  for (const e of picked) {
    jrnlState.status.set(e.batch_id, "pending");
    paintRowStatus(e);
  }
  paintAside(); // le bouton se désarme : ses entrées ne sont plus annulables

  let ok = 0;
  const failures: string[] = [];
  try {
    for (const e of picked) {
      try {
        await revertBatch(e.batch_id);
        jrnlState.status.set(e.batch_id, "reverted");
        jrnlState.failReason.delete(e.batch_id);
        ok++;
      } catch (err: unknown) {
        jrnlState.status.set(e.batch_id, "failed");
        const msg = humanRevertError(err);
        jrnlState.failReason.set(e.batch_id, msg);
        failures.push(msg);
      }
      paintRowStatus(e);
    }
  } finally {
    // `finally` et non une ligne après la boucle : un jet inattendu (un `paintRowStatus` sur un DOM
    // remplacé sous les pieds, par exemple) laisserait sinon l'écran incapable de rejouer une
    // annulation pour le reste de la session, sans rien dire.
    revertRunning = false;
  }

  paintAside();
  const done = `${plural(ok, "action")} annulée${ok > 1 ? "s" : ""}`;
  if (failures.length === 0) toast(done);
  else if (ok === 0) toast(failures[0]);
  else toast(`${done}, ${failures.length} en échec`);
}

// ---------------------------------------------------------------------------
// Souris, clavier, menu contextuel
// ---------------------------------------------------------------------------

async function openLocation(e: JournalEntry): Promise<void> {
  if (e.track_id == null) {
    toast("Emplacement inconnu — cette entrée n'est plus liée à une piste en base.");
    return;
  }
  try {
    await revealTrack(e.track_id);
  } catch (err: unknown) {
    toast(humanizeError(err, "Impossible d'ouvrir l'emplacement", "reveal_track"));
  }
}

/** Ouvre Bibliothèque filtrée sur le nom de la piste. La navigation passe par un clic sur la
 *  destination du rail, pas par un import du routeur : `router.ts` importe CE module, un import
 *  retour fermerait le cycle (`CLAUDE.md` § Modules frontend). */
function showInLibrary(e: JournalEntry): void {
  const name = trackName(e);
  if (!name || name === "—") return;
  bibState.filter = { q: name };
  document
    .querySelector<HTMLElement>('[data-view="biblio"]')
    ?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
}

function openJournalContextMenu(x: number, y: number, id: string): void {
  // Convention système : un clic droit HORS de la sélection la remplace ; DEDANS il la garde
  // entière. Sans cette règle, le même geste porterait tantôt sur une ligne, tantôt sur cent.
  if (!jrnlState.selection.has(id)) {
    applyJrnlClick(id, { shift: false, meta: false });
    syncSelectionUi();
  }
  const ids = [...jrnlState.selection];
  if (ids.length === 0) return;
  const picked = ids.map(entryById).filter((e): e is JournalEntry => !!e);
  const one = picked.length === 1 ? picked[0] : undefined;
  const applicable = picked.filter(revertible).length;
  const suffix = applicable > 1 ? ` (${applicable})` : "";
  const path = one ? (one.to_path ?? one.from_path) : null;
  // Une entrée sans action est DÉSACTIVÉE, pas retirée (`context-menu.ts`) : un menu dont les
  // entrées vont et viennent se relit à chaque ouverture.
  openContextMenu(x, y, [
    {
      label: `Annuler cette action${suffix}`,
      onPick: applicable ? () => void revertSelection() : undefined,
    },
    {
      label: "Ouvrir l'emplacement",
      separated: true,
      onPick: one && one.track_id != null ? () => void openLocation(one) : undefined,
    },
    { label: "Copier le chemin", onPick: path ? () => copyToClipboard(path, "Chemin copié") : undefined },
    {
      label: "Voir la piste dans Bibliothèque",
      onPick: one ? () => showInLibrary(one) : undefined,
    },
  ]);
}

/** Couche 2 du clavier (DESIGN.md § 9) : ↑ ↓ déplacent, ⇧+↑↓ étendent, Début/Fin vont aux bouts —
 *  par INDEX dans la liste ordonnée VISIBLE, jamais en marchant le DOM (un groupe replié n'a pas de
 *  nœuds, et le voisin DOM ne serait pas le voisin de liste). */
function stepJrnlSelection(key: string, shift: boolean): boolean {
  const ids = visibleIds();
  if (!ids.length) return false;
  const cursor = jrnlState.anchor != null ? ids.indexOf(jrnlState.anchor) : -1;
  let next: number;
  switch (key) {
    case "ArrowDown":
      next = cursor < 0 ? 0 : Math.min(ids.length - 1, cursor + 1);
      break;
    case "ArrowUp":
      next = cursor < 0 ? ids.length - 1 : Math.max(0, cursor - 1);
      break;
    case "Home":
      next = 0;
      break;
    case "End":
      next = ids.length - 1;
      break;
    default:
      return false;
  }
  const target = ids[next];
  if (shift && jrnlState.anchor != null) {
    // ⇧ étend depuis l'ancre SANS la déplacer — c'est ce qui permet de revenir en arrière dans la
    // même plage.
    selectRange(ids, ids.indexOf(jrnlState.anchor), next);
  } else {
    jrnlState.selection.clear();
    jrnlState.selection.add(target);
    jrnlState.anchor = target;
  }
  syncSelectionUi();
  rowEl(target)?.scrollIntoView({ block: "nearest" });
  return true;
}

/** Vrai quand la table du Journal est à l'écran. Les gestionnaires globaux ci-dessous sont posés
 *  UNE fois et vivent jusqu'à la fermeture de l'app : c'est cette garde, et non un
 *  ajout/retrait d'écouteur, qui les borne à cet écran.
 *
 *  Deux étages, et l'ordre EST le point. Ces gestionnaires reçoivent chaque clic, chaque frappe et
 *  chaque clic droit des six autres écrans : hors Journal, la garde tient maintenant en un test de
 *  booléen, sans toucher au DOM. Le `querySelector` ne tranche que sous drapeau levé — et le baisse
 *  définitivement dès que le routeur a rendu autre chose, si bien qu'un seul événement paie la
 *  sortie de l'écran. Le drapeau est posé par `loadAndPaint` et n'est jamais effacé de l'extérieur :
 *  aucun des six voisins ne connaît le Journal, et leur demander de le prévenir serait une
 *  obligation croisée qui pourrirait au premier écran ajouté. */
let journalPainted = false;

function journalOnScreen(): boolean {
  if (!journalPainted) return false;
  journalPainted = !!document.querySelector("#content .jrnl-wrap");
  return journalPainted;
}

let handlersInstalled = false;

function installJournalHandlers(): void {
  if (handlersInstalled) return;
  handlersInstalled = true;

  // CLIC — délégué sur `#content`, qui est permanent : les lignes, elles, sont recréées à chaque
  // rendu. Fréquence : un geste utilisateur.
  document.getElementById("content")?.addEventListener("click", (e: MouseEvent) => {
    if (!journalOnScreen()) return;
    const t = e.target as HTMLElement | null;
    if (!t) return;

    const grp = t.closest<HTMLElement>("[data-jgroup]");
    if (grp?.dataset.jgroup) {
      const k = grp.dataset.jgroup;
      if (jrnlState.collapsed.has(k)) jrnlState.collapsed.delete(k);
      else jrnlState.collapsed.add(k);
      repaintBody();
      return;
    }

    const row = t.closest<HTMLElement>(".jrnl-row[data-jrow]");
    const id = row?.dataset.jrow;
    if (!id) return;
    applyJrnlClick(id, { shift: e.shiftKey, meta: e.metaKey || e.ctrlKey });
    syncSelectionUi();
  });

  document.getElementById("content")?.addEventListener("dblclick", (e: MouseEvent) => {
    if (!journalOnScreen()) return;
    const id = (e.target as HTMLElement | null)?.closest<HTMLElement>(".jrnl-row[data-jrow]")?.dataset.jrow;
    const entry = id ? entryById(id) : undefined;
    if (entry) void openLocation(entry);
  });

  document.addEventListener("contextmenu", (e: MouseEvent) => {
    if (!journalOnScreen()) return;
    const id = (e.target as HTMLElement | null)?.closest<HTMLElement>(".jrnl-row[data-jrow]")?.dataset.jrow;
    if (!id) return;
    e.preventDefault();
    openJournalContextMenu(e.clientX, e.clientY, id);
  });

  // CLAVIER — couches 1 et 2 de DESIGN.md § 9. `shortcuts.ts` borne les siennes à la présence
  // d'une ligne `.lr[data-bib="row"]` (Bibliothèque) : les lignes du Journal n'en portent pas, donc
  // il laisse passer sans `preventDefault`, et ce gestionnaire — posé plus tard, donc appelé après —
  // les traite. ⌫ ne fait RIEN ici : supprimer une entrée d'historique n'a pas de sens, et la
  // touche est destructive ailleurs.
  document.addEventListener("keydown", (e: KeyboardEvent) => {
    // La garde de champ de saisie AVANT celle d'écran : les deux doivent passer, et celle-ci est la
    // moins chère — c'est elle qui absorbe la frappe dans la recherche de la barre, l'événement
    // clavier le plus fréquent de cet écran.
    const el = e.target as HTMLElement | null;
    if (el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable)) return;
    if (!journalOnScreen()) return;

    if (e.key === "ArrowDown" || e.key === "ArrowUp" || e.key === "Home" || e.key === "End") {
      if (e.ctrlKey || e.metaKey || e.altKey) return;
      if (stepJrnlSelection(e.key, e.shiftKey)) e.preventDefault();
      return;
    }
    if ((e.key === "a" || e.key === "A") && (e.ctrlKey || e.metaKey) && !e.altKey) {
      const ids = visibleIds();
      if (!ids.length) return;
      e.preventDefault();
      // Tout ce que le filtre laisse VOIR, jamais au-delà : ⌘A qui porterait sur des lignes hors
      // écran est le raccourci le plus dangereux qui soit.
      jrnlState.selection.clear();
      for (const id of ids) jrnlState.selection.add(id);
      jrnlState.anchor = ids[0];
      syncSelectionUi();
      return;
    }
    if (e.key === "Enter") {
      // Un en-tête de groupe focalisé est un vrai `<button>` : son Entrée native le replie déjà.
      // Sans cette garde, la même frappe ouvrirait EN PLUS l'emplacement de la ligne sélectionnée.
      if (el?.tagName === "BUTTON") return;
      const ids = [...jrnlState.selection];
      if (ids.length !== 1) return;
      const entry = entryById(ids[0]);
      if (!entry) return;
      e.preventDefault();
      void openLocation(entry);
    }
  });
}
