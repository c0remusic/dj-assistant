// Ligne de source du rail — markup pur, SANS import de `./ipc`, chargeable par Vitest en env Node
// et par Storybook (même séparation que `source-color.ts` et `popover-position.ts`). La story
// appelle CE rendu au lieu d'en recopier le markup — même motif que la refonte des stories du
// Journal du 2026-08-20 : une copie ne peut que diverger.
import type { Source } from "../shared/contracts";
import { esc } from "./dom";
import { resolveSourceColorKey } from "./source-color";

/** Dernier segment d'un chemin, séparateurs Windows et POSIX confondus. */
export function baseName(p: string): string {
  return p.split(/[\\/]/).filter(Boolean).pop() || p;
}

/** Une entrée de source. Même grammaire que les autres entrées du rail (`.nv`) : la section
 *  Sources n'est pas un composant à part, c'est le rail avec un contenu de plus. La pastille de
 *  couleur est un accent CATÉGORIEL — elle identifie la source ailleurs dans l'app, elle ne porte
 *  aucun état (DESIGN.md § 4). Teinte : override manuel sinon cycle par ordre d'ajout
 *  (`source-color.ts`) — jamais neutre : un gris uniforme n'identifierait rien.
 *
 *  États (rail.md § États) : l'échec PRIME sur la suspension — « jamais atténuée », un échec se
 *  voit mieux que le reste, pas moins bien — donc `--suspended` ne se pose que sans `--error`.
 *  La suspension ne touche pas l'encre de la ligne : le repos de `.nv` est déjà
 *  `--color-text-tertiary`, la valeur que la spec prescrit. Son signal est la pastille VIDÉE
 *  (contour sans fond, teinte conservée — voir `styles.css` § section Sources). */
export function sourceEntryHtml(
  s: Source,
  all: Source[],
  active: boolean,
  failure: string | undefined,
): string {
  const r = railRowState(s, all, active, failure);
  return (
    `<div class="${esc(r.rowClass)}" data-src="${r.id}" tabindex="0" role="button" title="${esc(r.title)}">` +
    `<span class="${esc(r.dotClass)}" aria-hidden="true"></span>` +
    `<span>${esc(r.label)}</span>` +
    `<span class="nav-badge">${esc(r.badge)}</span></div>`
  );
}

/** Ce qu'une ligne de source doit MONTRER, sous forme de données et non de markup.
 *
 *  Une seule description, deux consommateurs : `sourceEntryHtml` ci-dessus la sérialise quand il
 *  faut créer les nœuds, et `rail-sources.ts` l'applique aux nœuds DÉJÀ en place quand il faut
 *  seulement les mettre à jour. C'est ce qui empêche les deux chemins de diverger — le rendu
 *  initial et la mutation ne peuvent pas dire deux choses différentes s'ils lisent la même
 *  fonction. Même motif que la story, qui appelle le vrai rendu plutôt que d'en recopier le markup.
 *
 *  ⚠️ Les champs texte sont BRUTS, jamais pré-échappés. `esc()` appartient au chemin HTML, qui
 *  interpole dans des attributs et du contenu ; le chemin mutation affecte `textContent`,
 *  `title` et `className`, trois propriétés DOM qui ne parsent rien et pour lesquelles un
 *  échappement produirait des entités VISIBLES à l'écran. Pré-échapper ici casserait donc la
 *  moitié mutation, en silence. */
export interface RailRowState {
  id: number;
  /** Nom affiché — dernier segment du chemin. */
  label: string;
  /** Texte du badge, VIDE (et non absent) quand rien n'est en attente : `.nav-badge:empty`
   *  (`styles.css`) replie le compte. Un span toujours présent se met à jour par `textContent`,
   *  sans jamais créer ni détruire de nœud — c'est ce qui rend la ligne mutable.
   *
   *  Les SOURCES sont les seules entrées de rail à porter un compte depuis le 2026-08-26 : celui
   *  de Revue est retiré, son nombre étant déjà écrit dans la barre à côté du titre. La règle et
   *  les refs qui la donnent : `docs/ui-specs/rail.md` § Item de navigation. */
  badge: string;
  /** Classe complète de la pastille, teinte comprise. */
  dotClass: string;
  /** Classe complète de la ligne : grammaire `.nv`, marqueur actif, état. */
  rowClass: string;
  /** Infobulle. */
  title: string;
}

/** États (rail.md § États) : l'échec PRIME sur la suspension — « jamais atténuée » — donc
 *  `--suspended` ne se pose que sans `--error`. Le VIDE (`track_count === 0`, issue #55,
 *  décision B2 du 2026-09-02) s'intercale : échec > vide > suspension. Le vide est une
 *  information corrective — l'utilisateur a probablement pointé le mauvais dossier — quand la
 *  suspension n'est qu'un choix ; le badge « 0 audio » (encre warning, pastille éteinte par CSS)
 *  doit se lire même sur une source suspendue. */
export function railRowState(
  s: Source,
  all: Source[],
  active: boolean,
  failure: string | undefined,
): RailRowState {
  const broken = !s.accessible || failure != null;
  const empty = !broken && s.track_count === 0;
  const suspended = !s.watched && !broken && !empty;
  const state = broken
    ? " sift-rail-src--error"
    : empty
      ? " sift-rail-src--empty"
      : suspended
        ? " sift-rail-src--suspended"
        : "";
  return {
    id: s.id,
    label: baseName(s.path),
    badge: empty ? "0 audio" : s.pending_count > 0 ? String(s.pending_count) : "",
    dotClass: `sift-rail-src-dot sift-rail-src-dot-${resolveSourceColorKey(all, s)}`,
    rowClass: `nv sift-rail-src${active ? " on" : ""}${state}`,
    title: !s.accessible
      ? `${s.path} — dossier inaccessible`
      : failure
        ? `${s.path} — scan en échec : ${failure}`
        : empty
          ? `${s.path} — aucun fichier audio reconnu`
          : suspended
            ? `${s.path} — surveillance suspendue`
            : s.path,
  };
}

/** Identité et ORDRE des lignes — tout ce qu'une mise à jour en place ne sait PAS rattraper.
 *
 *  Le compte en attente, l'état, la teinte et l'infobulle n'en font délibérément pas partie : ce
 *  sont précisément les champs qui bougent à chaque tick de scan, et les muter sur place est le
 *  but. Ne rentre ici que ce qui exige de créer ou de retirer un nœud — donc les identifiants,
 *  dans leur ordre d'affichage.
 *
 *  Deux listes de même clé sont interchangeables ligne à ligne : `rail-sources.ts` peut alors
 *  garder ses nœuds et n'écrire que les valeurs. Clé différente = reconstruction. */
export function railShapeKey(sources: Source[]): string {
  return sources.map((s) => s.id).join(",");
}
