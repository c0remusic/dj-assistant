import { undoLast } from "./ipc";
import { esc } from "./dom";
import { humanizeError } from "./errors";

let clearPaneHook: ((mid: HTMLElement) => void) | null = null;

/** Registered once by filing.ts at module load (mirrors filing-bins.ts's
 *  registerOpenTrackPathGetter/registerDestChangeHook) — lets toast()'s default (LIFO) undo
 *  fallback clear the detail pane without this module importing filing.ts back (would be a
 *  static import cycle: filing.ts needs toast() too). */
export function registerClearPaneHook(hook: (mid: HTMLElement) => void): void {
  clearPaneHook = hook;
}

let toastTimer: number | undefined;

/** Exit fade length. Must stay in step with `--duration-fast` in styles.css: this timer is what
 *  actually removes the node, the CSS only paints the fade. */
const TOAST_EXIT_MS = 75;

/** A transient toast at the bottom-right with an optional "Undo" action. With `onUndo` the Undo
 *  button runs that callback (e.g. a targeted revert of a specific batch); without it, Undo falls
 *  back to `undoLast` (the LIFO most-recent action), clears the detail pane, and reports the
 *  outcome (done / nothing to undo / failed) in a follow-up toast. `undo` defaults to
 *  false: callers that only need a plain message call `toast("…")`.
 *  A single toast exists at a time, by construction: when OUR `#sift-toast` is already on screen its
 *  content is MUTATED and the dismiss timer restarted, instead of removing then recreating the
 *  node.
 *
 *  `actionLabel` renomme ce bouton quand l'action n'est pas une annulation — ajouté le 2026-09-02
 *  pour le toast de blocage « aucune racine de bibliothèque » (issue #54), qui doit proposer
 *  « Choisir la racine ». Le MÉCANISME existait déjà en entier (bouton + callback) : seul son
 *  libellé était figé sur « Annuler ». Il reste le défaut, donc aucun appelant existant ne bouge,
 *  et la classe/`data-fil` du bouton ne changent pas (la feuille les vise). */
export function toast(
  message: string,
  undo = false,
  onUndo?: () => void,
  actionLabel = "Annuler",
): void {
  // Ce module est le SEUL à construire `#sift-toast`. Le garde `dataset.owner` qui vivait ici
  // n'existait que pour se protéger du toast privé de `library-detail.ts`, une copie de cette
  // fonction avec son propre timer de 6 s dont l'id n'était jamais mémorisé — donc impossible à
  // annuler d'ici, et capable de retirer à SON échéance un nœud qu'on venait de réutiliser. Ce
  // rival a été supprimé au profit d'un import : le garde n'a plus rien à garder.
  const existing = document.getElementById("sift-toast");
  const el = existing ?? document.createElement("div");
  if (!existing) {
    el.id = "sift-toast";
    el.className = "sift-toast";
    el.setAttribute("role", "status");
    el.setAttribute("aria-live", "polite");
  }
  el.innerHTML =
    `<span>${esc(message)}</span>` +
    (undo
      ? `<button data-fil="undo" class="sift-toast-undo">${esc(actionLabel)}</button>`
      : "");
  if (!existing) {
    document.body.appendChild(el);
    // Fade in on CREATION ONLY. Never on the mutation path: re-fading a toast that is already on
    // screen would blink the message the user is in the middle of reading.
    // The from-state is dropped on the SECOND animation frame — a rAF callback runs BEFORE style
    // recalc in Chromium/WebView2, so removing it on the first frame means the node never held a
    // computed opacity:0 and the transition silently never plays.
    el.classList.add("sift-fade-in", "sift-fade-from");
    requestAnimationFrame(() => requestAnimationFrame(() => el.classList.remove("sift-fade-from")));
  }
  el.querySelector('[data-fil="undo"]')?.addEventListener("click", () => {
    el.remove();
    if (onUndo) {
      onUndo(); // targeted revert (e.g. revertBatch of THIS tag_edit) — pane stays as-is
      return;
    }
    void undoLast()
      .then((batchId) => {
        // the just-filed track is back in the queue — clear the stale detail pane
        const mid = document.getElementById("mid");
        if (mid) clearPaneHook?.(mid);
        // Visible outcome, same wording as doRevert (filing-actions.ts). Re-entering toast() from
        // toast()'s own handler is safe: this runs AFTER the promise settles, the follow-up toast
        // is built with `undo=false` (no button → no new handler → no recursion), and it never
        // writes to `el` — that node was detached above, and toast() re-reads #sift-toast from the
        // document, so it just creates a fresh one (or mutates whatever toast is on screen by then).
        // `undoLast` resolves null when the journal had nothing live to revert: say so rather than
        // claim an undo that did not happen.
        toast(batchId ? "Annulé — retour dans la file" : "Rien à annuler.", false);
      })
      .catch((e) => {
        // Même raison qu'à `filing-actions.ts::doRevert` : le message de domaine devient le
        // `display`, donc les deux branches sont journalisées.
        const msg = String(e);
        toast(
          humanizeError(
            e,
            msg.includes("source gone")
              ? "Annulation impossible : un fichier nécessaire a disparu — l'original a peut-être été purgé de la corbeille."
              : "Échec de l'annulation — réessaie",
            "undoLast",
          ),
          false,
        );
      });
  });
  clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => {
    // Drop the id BEFORE painting the exit: for the whole exit window this node is still in the
    // document, and a toast() call landing in it must NOT find it via getElementById and mutate a
    // node that is on its way out (the message would appear, then fade away with it). Without an
    // id, that call takes the create path and builds a fresh toast — the dying one just finishes
    // dying.
    el.removeAttribute("id");
    el.classList.add("sift-fade-out");
    // ONE removal path, a timer — never `transitionend`. That event does not fire when the
    // transition is cancelled or never starts (reduced motion, or the node detached meanwhile),
    // and the toast would then stay on screen forever.
    window.setTimeout(() => el.remove(), TOAST_EXIT_MS);
  }, 6000);
}

/** Écrit `text` dans le presse-papier et dit ce qui s'est passé — les deux issues, jamais une seule.
 *
 *  Ici parce que le retour est un toast, et que le toast vit ici : ce module ne dépend de rien
 *  d'autre que d'`./ipc` et d'`./errors`, donc tout écran peut l'appeler sans créer de cycle.
 *
 *  `okToast` n'est PAS peint avant l'écriture. C'est la leçon du site Écartés (`sift-live.ts`), où
 *  la coche « Copié » était posée d'avance et le `catch` vide : un refus du presse-papier
 *  (permission, focus perdu) affichait quand même le succès, et l'utilisateur collait l'ancien
 *  contenu sans savoir pourquoi. L'échec est donc dit à l'écran ET journalisé — `errors.ts` garantit
 *  la chaîne brute en console, le toast n'en donne que la version lisible.
 *
 *  Le succès se dit par un toast et non par un changement dans le bouton cliqué : une seule
 *  grammaire de retour pour « copié », valable même quand le bouton a disparu entre le clic et la
 *  résolution (tout changement d'écran reconstruit `#content` par `innerHTML`). */
export function copyToClipboard(text: string, okToast: string): void {
  void navigator.clipboard
    .writeText(text)
    .then(() => toast(okToast))
    .catch((err: unknown) => {
      console.error("clipboard writeText failed", err);
      toast("Copie impossible — le presse-papier a refusé");
    });
}
