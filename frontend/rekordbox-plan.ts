// Plan de synchronisation Rekordbox — module pur, sans DOM ni IPC, testable en env Node.
//
// « Pourquoi on ne peut pas tout synchroniser d'un coup ? » (Antoine, 2026-09-08). Rien de
// technique ne l'empêchait : quatre écritures existent côté Rust, chacune avec son backup et son
// refus si Rekordbox tourne. Ce qui manquait, c'était l'écran — trois chantiers M8 livrés l'un
// après l'autre, chacun avec son bouton. Ce module décide QUOI écrire, dans l'ordre des tiers ;
// `rekordbox-view.ts` exécute et rend compte.
//
// Deux portées, celles de Photos › Importer (« Import Selected » / « Import All New Photos ») :
// `selection` = ce qui est coché, `all` = tout ce qui est en attente. Les deux se bornent à la
// section active de la colonne (« Tout » = les quatre) — choisir une section, c'est le bouton de
// contenu du Finder (General · Music · Movies…) : l'action porte sur ce qu'on regarde.

export type RkbSection = "all" | "files" | "meta" | "art" | "dedup";

export interface SyncPending {
  /** Ids Tier 1 (corrections de chemin) en attente — jamais les ambigus. */
  repairs: readonly number[];
  /** Ids Tier 3 métadonnées en attente. */
  metas: readonly number[];
  /** Ids Tier 3 pochettes en attente. */
  arts: readonly number[];
  /** Clés `playlist_id::content_id` des groupes de doublons (Tier 2). */
  dedups: readonly string[];
}

export interface SyncSelection {
  repairs: ReadonlySet<number>;
  metas: ReadonlySet<number>;
  arts: ReadonlySet<number>;
  dedups: ReadonlySet<string>;
}

export interface SyncPlan {
  repairs: number[];
  metas: number[];
  arts: number[];
  dedups: string[];
  total: number;
}

function within<T>(ids: readonly T[], sel: ReadonlySet<T> | null): T[] {
  return sel ? ids.filter((id) => sel.has(id)) : [...ids];
}

/** Ce que « Tout synchroniser » (`scope: "all"`) ou « Synchroniser la sélection »
 *  (`scope: "selection"`) écrira, borné à la section `active`. Une sélection ne peut viser que ce
 *  qui est encore en attente : un id coché puis appliqué ailleurs tombe du plan sans bruit. */
export function planSync(
  active: RkbSection,
  pending: SyncPending,
  selection: SyncSelection,
  scope: "all" | "selection",
): SyncPlan {
  const sel = scope === "selection";
  const on = (key: Exclude<RkbSection, "all">) => active === "all" || active === key;
  const repairs = on("files") ? within(pending.repairs, sel ? selection.repairs : null) : [];
  const metas = on("meta") ? within(pending.metas, sel ? selection.metas : null) : [];
  const arts = on("art") ? within(pending.arts, sel ? selection.arts : null) : [];
  const dedups = on("dedup") ? within(pending.dedups, sel ? selection.dedups : null) : [];
  return { repairs, metas, arts, dedups, total: repairs.length + metas.length + arts.length + dedups.length };
}
