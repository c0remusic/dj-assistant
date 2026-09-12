import type { Meta, StoryObj } from "@storybook/html-vite";
import type { JournalEntry } from "../shared/contracts";
import {
  theadHtml,
  skeletonHtml,
  rowHtml,
  groupHtml,
  type JrnlGroup,
  type JrnlRowView,
  type JrnlStatus,
} from "./journal";

// La table du Journal, refondue le 2026-08-19 (`frontend/journal.ts`, spec
// `docs/ui-specs/journal.md`). L'ancienne liste `<details>` par catégorie est partie avec toutes
// ses classes — `.jrnl-cat*`, `.jrnl-revert`, `.jrnl-toast`, `.jrnl-banner`, `.jrnl-qrow`,
// `.jrnl-mass`, `.jrnl-voir-tout` : voir le bloc « REFONTE DU 2026-08-19 » en tête de la section
// Journal de `frontend/styles.css`, qui nomme chaque classe retirée et son remplaçant.
//
// Ces stories EXÉCUTENT le vrai rendu. Jusqu'au 2026-08-20 elles en RECOPIAIENT ~90 lignes —
// en-tête, ligne, groupe, squelette, table des libellés d'état — parce que tout le markup de
// `journal.ts` était privé et lisait `jrnlState`. Une copie ne peut que diverger, et les douze
// numéros de ligne qu'elle citait pour se rattacher à l'original étaient déjà faux. Les quatre
// fonctions sont maintenant publiques et PURES : ce qu'elles savaient de la vue (sélection, statut,
// racine de bibliothèque, repli) leur arrive en argument. Modèle : `library-verdict.stories.ts`,
// qui appelle `libraryTableRowHtml` pour la même raison.
//
// Ce que la story fournit est donc de la DONNÉE (`JournalEntry`) plus un `JrnlRowView`, et tout le
// reste est dérivé par le code réel : le libellé d'action vient de `kind`, le nom de piste et la
// destination du chemin, l'heure de `ts`. Deux conséquences visibles dans les contrôles :
//   · `ts` est l'horodatage BRUT de SQLite, en UTC (`YYYY-MM-DD HH:MM:SS`) — la cellule affiche
//     l'heure LOCALE du navigateur, donc « 19:47:00 » se peint « 21:47 » à Paris l'été. C'est
//     exactement la conversion que `parseTs` existe pour garantir.
//   · la destination n'est pas réglable seule : c'est `to_path` moins `root` quand la racine le
//     préfixe (`relDest`), et les deux derniers segments sinon.
//
// Aucune couleur, aucune taille ici : toutes les classes viennent de `frontend/styles.css`, et
// c'est elle seule qui peint.
//
// ÉTAT NON REPRÉSENTABLE ICI : `.jrnl-row--flash`, le flash vert de la transition « annulé ». Il
// est posé puis RETIRÉ à `animationend` par `paintRowStatus` — une story statique ne peut que le
// montrer éteint. Seule la transition se colore ; l'état annulé permanent, lui, est celui de la
// story `Annulee` (encre tertiaire, jamais une teinte).

/** « Appliqué » n'est pas un statut : c'est l'ABSENCE de statut (`JrnlStatus | null`). Le contrôle
 *  le nomme quand même, parce qu'un bouton radio sans quatrième option cacherait l'état le plus
 *  courant de la table. */
type RowStatus = "applied" | JrnlStatus;

interface RowArgs {
  /** Horodatage BRUT de la base, UTC. Voir l'en-tête : la cellule montre l'heure locale. */
  ts: string;
  /** Les QUATRE `kind` du contrat. `actionLabel` en tire « Rangé » (convert/move), « Purgé »
   *  (trash), « Écarté » (reject) — aucun autre libellé n'est atteignable, voir l'écart spec↔réel
   *  noté sur cette fonction. */
  kind: JournalEntry["kind"];
  /** Morceaux du lot. La marque `×N` n'est peinte qu'au-delà de 1. */
  track_count: number;
  /** Le chemin écrit. Il porte à lui seul le nom de piste (`basenameNoExt`), la destination
   *  (`relDest`) et l'infobulle de la cellule. */
  to_path: string;
  from_path: string;
  /** Racine de bibliothèque. La destination en est le suffixe quand elle préfixe `to_path`. */
  root: string;
  status: RowStatus;
  selected: boolean;
}

const BASE: RowArgs = {
  ts: "2026-08-19 19:47:00",
  kind: "convert",
  track_count: 1,
  // La destination commence par une année entre parenthèses EXPRÈS : la cellule tronque par la
  // gauche (`direction:rtl`), et sans le `<bdi>` de `rowHtml` ce segment initial neutre remonterait
  // en fin de ligne — « The Universal Sky … (2002) ». La story le montrerait tout de suite.
  to_path: "D:/Musique/Techno/Marcel Dettmann/(2010) Dettmann/01 Marcel Dettmann - Seduction.aiff",
  from_path: "D:/Inbox/marcel dettmann - seduction.flac",
  root: "D:/Musique",
  status: "applied",
  selected: false,
};

function entry(a: RowArgs, id: string): JournalEntry {
  return {
    batch_id: id,
    track_id: 1,
    kind: a.kind,
    from_path: a.from_path,
    to_path: a.to_path,
    ts: a.ts,
    session_id: null,
    track_count: a.track_count,
    // `rowHtml` prend son statut de `view` et ne lit jamais la donnée : `statusOf` — qui arbitre
    // entre la carte locale et ce champ — vit du côté de la vue, hors de ce que la story peint.
    undone: false,
  };
}

function view(a: RowArgs): JrnlRowView {
  return { selected: a.selected, status: a.status === "applied" ? null : a.status, root: a.root };
}

function row(a: RowArgs, id = "b-1"): string {
  return rowHtml(entry(a, id), view(a));
}

/** Un groupe de session tel que `buildGroups` le fabrique, rendu par le vrai `groupHtml`.
 *  `count` permet d'afficher un gros groupe replié — l'en-tête compte `g.entries.length`, donc la
 *  liste est complétée par des copies qui ne sont jamais rendues (corps vidé au repli). */
function group(key: string, label: string, rows: RowArgs[], open: boolean, count = rows.length): string {
  const entries = rows.map((r, i) => entry(r, `${key}-${i}`));
  while (entries.length < count) entries.push(entries[0]);
  const g: JrnlGroup = { key, label, entries, children: [] };
  return groupHtml(g, 1, open, open ? rows.map((r, i) => row(r, `${key}-${i}`)).join("") : "");
}

/** `.jrnl-wrap` est OBLIGATOIRE autour de tout : c'est lui qui déclare `--jrnl-col-time`,
 *  `--jrnl-col-act` et `--jrnl-col-state`, les trois largeurs de colonnes fixes. Hors de ce
 *  wrapper, les trois `width:var(…)` ne résolvent rien et la table se disloque.
 *
 *  La largeur du cadre, elle, appartient à la STORY et non au composant : dans la vraie fenêtre la
 *  zone C est bornée par le rail et l'inspecteur (`--rail-w` 200px + `--aside-w` 320px, tokens de
 *  `styles.css`), soit ~760px sur une fenêtre de 1280. Une largeur finie est aussi ce qui rend
 *  visible la troncature par la gauche de la colonne Destination. */
function wrap(inner: string): HTMLElement {
  const el = document.createElement("div");
  el.className = "jrnl-wrap";
  el.style.maxWidth = "760px";
  el.innerHTML = inner;
  return el;
}

/** Rejoue le seul geste que la story a besoin de montrer : replier/déplier. Même effet que le
 *  gestionnaire délégué de `installJournalHandlers`, qui bascule la clé dans `jrnlState.collapsed`
 *  puis repeint le corps. Fréquence : un clic. */
function wireGroups(root: HTMLElement): void {
  root.querySelectorAll<HTMLButtonElement>(".jrnl-group-hd").forEach((hd) => {
    const body = hd.nextElementSibling as HTMLElement | null;
    if (!body) return;
    const rows = body.innerHTML;
    hd.addEventListener("click", () => {
      const open = hd.getAttribute("aria-expanded") === "true";
      hd.setAttribute("aria-expanded", open ? "false" : "true");
      body.innerHTML = open ? "" : rows;
    });
  });
}

const meta: Meta<RowArgs> = {
  title: "États de contenu/Journal — ligne de table",
  render: (args) => wrap(theadHtml() + `<div class="jrnl-body">${row(args)}</div>`),
  argTypes: {
    status: { control: "radio", options: ["applied", "pending", "reverted", "failed"] },
    selected: { control: "boolean" },
    kind: { control: "radio", options: ["convert", "move", "trash", "reject"] },
    track_count: { control: "number" },
    ts: { control: "text" },
    to_path: { control: "text" },
    from_path: { control: "text" },
    root: { control: "text" },
  },
  args: BASE,
};

export default meta;
type Story = StoryObj<RowArgs>;

/** Cas nominal. La ligne réutilise `.lr`, la ligne de table de l'app — hauteur, survol, filet,
 *  focus : une seule grammaire de ligne pour Bibliothèque et Journal. */
export const Normale: Story = {};

/** Sélection : `.sel`, promue au plan du contenu comme tout état actif de l'app. Jamais un accent
 *  coloré — un état permanent reste neutre. C'est aussi la ligne que l'inspecteur (zone D) détaille
 *  et depuis laquelle « Annuler » est offert : le bouton a quitté la ligne le 2026-08-19. */
export const Selectionnee: Story = { args: { selected: true } };

/** Annulation en cours : TRANSITOIRE, donc elle a le droit de se colorer — encre `info` sur la
 *  seule cellule État. La ligne reste utilisable, et le reste de la table aussi ; rien n'est
 *  désactivé au-delà du bouton de l'inspecteur. */
export const AnnulationEnCours: Story = { args: { status: "pending", track_count: 12 } };

/** Annulé : PERMANENT, donc neutre. L'encre de la piste et de l'état baisse d'un cran
 *  (`--color-text-tertiary`), elle ne s'éteint pas — aucune opacité, jamais. */
export const Annulee: Story = { args: { status: "reverted" } };

/** Échec d'annulation : encre `danger` sur piste, destination et état. Un échec ne s'estompe
 *  jamais, et son motif est lisible dans l'inspecteur (`.jrnl-insp-fail`). */
export const Echec: Story = { args: { status: "failed" } };

/** Les cinq d'un coup, dans une vraie table : c'est là qu'on vérifie que seul l'état transitoire
 *  porte une couleur, et que sélection et statut se cumulent sans se contredire. */
export const TousLesEtats: Story = {
  render: () =>
    wrap(
      theadHtml() +
        `<div class="jrnl-body">` +
        row(BASE, "b-1") +
        row(
          {
            ...BASE,
            ts: "2026-08-19 19:44:00",
            to_path: "D:/Musique/House/Levon Vincent/(2015) Live/02 Levon Vincent - Man or Mistress.aiff",
            selected: true,
          },
          "b-2",
        ) +
        row(
          {
            ...BASE,
            ts: "2026-08-19 19:39:00",
            kind: "reject",
            to_path: "D:/Musique/_ecartes/DJ Rashad - Feelin.mp3",
            status: "pending",
          },
          "b-3",
        ) +
        row(
          {
            ...BASE,
            ts: "2026-08-19 19:31:00",
            to_path: "D:/Musique/House/Anthony Naples/(2013) EP/01 Anthony Naples - Mad Disrespect.aiff",
            status: "reverted",
          },
          "b-4",
        ) +
        row(
          {
            ...BASE,
            ts: "2026-08-19 19:22:00",
            kind: "trash",
            to_path: "D:/Musique/_corbeille/Untitled - 03 (dup).flac",
            status: "failed",
          },
          "b-5",
        ) +
        `</div>`,
    ),
};

/** Groupes de session : déplié et replié. L'en-tête est un vrai `<button>` — Entrée et Espace
 *  marchent sans câblage — et le chevron pivote sur `[aria-expanded="true"]`, en `transform` seul.
 *  Cliquer bascule, comme dans l'app.
 *
 *  Le niveau 2 (`.jrnl-group--l2`, une session sous un jour) n'apparaît qu'en mode « Tout
 *  l'historique » : même en-tête, indenté d'un cran (`--space-12`), avec le libellé court
 *  « Session de 21h47 » au lieu de la forme datée. */
export const GroupeSession: Story = {
  render: () => {
    const rows = [
      BASE,
      {
        ...BASE,
        ts: "2026-08-19 19:44:00",
        to_path: "D:/Musique/House/Levon Vincent/(2015) Live/02 Levon Vincent - Man or Mistress.aiff",
      },
    ];
    const root = wrap(
      theadHtml() +
        `<div class="jrnl-body">` +
        group("s:1", "Session du 19/08/2026 21h30", rows, true) +
        group("s:2", "Session du 19/08/2026 18h05", rows, false, 47) +
        `</div>`,
    );
    wireGroups(root);
    return root;
  },
};

/** Chargement : le squelette occupe la structure FINALE — mêmes colonnes, même hauteur de ligne —
 *  pour que l'arrivée des données ne déplace rien. Pas d'animation : un squelette qui pulse attire
 *  l'œil sur une attente d'une centaine de millisecondes. Il n'est peint que si RIEN de valide
 *  n'est déjà à l'écran (garde de `loadAndPaint`). */
export const Squelette: Story = {
  render: () => wrap(theadHtml() + skeletonHtml()),
};
