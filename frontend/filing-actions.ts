import { fileTrack, listQueue, rejectTrack, requeueTrack, revertBatch } from "./ipc";
import type { QueueItem, TrackFileOutcome } from "../shared/contracts";
import { FILE_IN_PLACE } from "../shared/contracts";
import { esc } from "./dom";
import { confirmAction } from "./confirm-modal";
import { fileInPlaceChecked, getBinRel, binLabel } from "./filing-bins";
import {
  state,
  openState,
  ensureFilingWatcher,
  markFilingStarted,
  isFilingInFlight,
  filingFailure,
  onFilingOutcome,
  type InFlightFiling,
} from "./filing-state";
import { toast } from "./filing-toast";
import { humanizeError } from "./errors";

/** Banner label when a track was filed in place (its own source folder, not a tree bin). */
const IN_PLACE_BIN_LABEL = "source folder";

/** Ouvre l'écran Réglages — injecté par `sift-live.ts` (`registerOpenSettingsAction`), jamais
 *  importé : ce module ne peut pas importer `router.ts` sans refermer un cycle statique
 *  (router → queue-panel → filing → filing-actions). Même motif que `registerAddSourceAction`.
 *  Sert au toast actionnable du refus `NoLibraryRoot` (issue #54). */
let openSettings: (() => void) | null = null;
export function registerOpenSettingsAction(fn: () => void): void {
  openSettings = fn;
}

/** Va à Réglages, ou DIT que le câblage manque. Un bouton d'action qui ne fait rien est le pire
 *  des deux mondes : l'utilisateur croit avoir agi. Si l'injection n'a pas eu lieu (le wiring live
 *  n'a pas tourné — hors Tauri, ou un ordre de câblage cassé), la chaîne brute part en console
 *  plutôt qu'en silence, conformément au contrat d'`errors.ts`. Exportée parce que le mode Lot
 *  (`batch-panel.ts`) sert le MÊME refus et doit y mener par le même chemin. */
export function openSettingsScreen(): void {
  if (!openSettings) {
    console.error(
      "openSettingsScreen: aucune navigation vers Réglages injectée (registerOpenSettingsAction) — le bouton « Choisir la racine » ne mène nulle part",
    );
    return;
  }
  openSettings();
}

/** Hôte du bandeau « Rangé » : la rangée réglages de la BOÎTE de lecture en mode Détail
 *  (`#filbox-settings`, décision V2b du 2026-08-30), avec repli sur le pied de panneau `#filfoot`
 *  — le seul hôte qui existe en mode Lot, et le seul qui existait avant cette décision.
 *  Le bandeau se pose en TÊTE de cet hôte, au-dessus des contrôles qu'il commente. */
function filedBannerHost(): HTMLElement | null {
  return document.getElementById("filbox-settings") ?? document.getElementById("filfoot");
}

/** Le bandeau lui-même, où qu'il vive. Requête document-wide : il n'y en a qu'un à la fois
 *  (showFiledConfirm retire le précédent avant d'en poser un neuf). */
function filedBannerEl(): HTMLElement | null {
  return document.querySelector<HTMLElement>(".sift-filed-banner");
}

/** Disable/enable the rail action buttons (visible feedback while an action runs). The buttons
 *  live outside the #mid pane's report subtree, so query the document rather than that pane. */
function setActionsDisabled(disabled: boolean): void {
  document
    .querySelectorAll<HTMLButtonElement>('[data-fil="ranger"],[data-fil="resource"],[data-fil="trash"]')
    .forEach((b) => {
      // État disabled porté par le CSS token (`:disabled` dans styles.css), pas par une opacité
      // inline — DESIGN.md:262 interdit l'atténuation d'état à l'opacité. `b.disabled` bloque déjà
      // le clic ; le `pointer-events:none` inline était redondant.
      b.disabled = disabled;
    });
}

/** Ranger the current track into the selected bin.
 *
 *  ASYNCHRONOUS since P5 (PRD 2026-07-27, D3/D5): `fileTrack` no longer resolves when the file is
 *  converted, it resolves when the conversion has been PLANNED and started — destination path and
 *  journal batch id settled, ffmpeg still running. So the click gives the rail back immediately and
 *  the pane advances to the next track while the previous one converts. The refusals that must be
 *  answered before anything starts (RAIL_MISMATCH's confirmation, NoLibraryRoot, upscale) still
 *  come back from this very call, so the retry loop below is unchanged. What lands later — success
 *  or failure — arrives on `file:track:done` and is handled by `settleFilingBanner`. */
export async function doRanger(
  mid: HTMLElement,
  openNext: (mid: HTMLElement, item: QueueItem) => Promise<void>,
  clearPane: (mid: HTMLElement, emptyQueue?: boolean) => void,
): Promise<void> {
  if (!state.track || !state.canonical || openState.acting) return;
  const track = state.track;
  const canonical = state.canonical;
  const inPlace = fileInPlaceChecked();
  const dest = inPlace ? FILE_IN_PLACE : getBinRel();
  if (dest === null) {
    toast("Choisis un dossier de destination.", false);
    return;
  }
  const ranger = document.querySelector<HTMLElement>('[data-fil="ranger"]');
  const orig = ranger?.innerHTML ?? null;
  // Subscribe before launching anything, and AWAIT the subscription: the outcome event must have a
  // live listener by the time the backend can emit it (a very short conversion can settle almost
  // immediately, and `listen()` is an invoke with no ordering guarantee against `file_track`).
  await ensureFilingWatcher();
  openState.acting = true;
  setActionsDisabled(true);
  if (ranger)
    ranger.innerHTML =
      '<i class="ti ti-loader-2 sift-spin sift-icon-inline-md"></i> Conversion en cours…';
  let allowRailMismatch = false;
  try {
    for (;;) {
      try {
        const res = await fileTrack(track.id, dest, state.target, canonical, allowRailMismatch);
        const filedPath = res.path;
        const batchId = res.batch_id;
        const bin = inPlace ? IN_PLACE_BIN_LABEL : binLabel();
        // The conversion is now running behind us. Registering it here is what takes the track out
        // of the loop: it is still `pending` in the DB, so listQueue below (and every later render)
        // would otherwise hand it straight back and let it be converted a second time.
        markFilingStarted(track.id, {
          batchId,
          path: filedPath,
          bin,
          name: filedPath.split(/[\\/]/).pop() || filedPath,
        });
        let items: QueueItem[] = [];
        try {
          items = (await listQueue()).filter((it) => !isFilingInFlight(it.id));
        } catch (err) {
          console.error("listQueue failed after filing", err);
        }
        if (items.length) await openNext(mid, items[0]);
        else clearPane(mid, true);
        showFiledConfirm(track.id, batchId, bin, filedPath);
        return;
      } catch (e) {
        const msg = String(e);
        if (!allowRailMismatch && msg.includes("RAIL_MISMATCH")) {
          const ext = (track.path.split(".").pop() || "").toUpperCase();
          const proceed = await confirmAction(
            `Ce fichier est déclaré ${ext} mais son contenu réel est compressé (lossy) — ` +
              `le convertir créerait un faux fichier lossless.\n\nConvertir quand même ?`,
          );
          if (proceed) {
            allowRailMismatch = true;
            continue;
          }
          setActionsDisabled(false);
          if (ranger && orig != null) ranger.innerHTML = orig;
          return;
        }
        throw e;
      }
    }
  } catch (e) {
    const msg = String(e);
    // Toast ACTIONNABLE depuis le 2026-09-02 (issue #54, direction A1 pour ce point) : le refus
    // nommait un réglage manquant sans y mener, et c'est exactement le manque que #16 relevait.
    // Depuis le volet backend du même ticket, ce refus ne peut plus venir que d'une destination
    // qui vise l'ARBRE — d'où la formulation, qui ne prétend plus que toute conversion est bloquée.
    if (msg.includes("NoLibraryRoot"))
      toast(
        "Conversion bloquée — aucune racine de bibliothèque.",
        true,
        openSettingsScreen,
        "Choisir la racine",
      );
    // The backend refuses a second filing of a track whose conversion is still running (P5). The
    // front normally hides such a track from the queue, so reaching this means it came back through
    // a path that doesn't go through the queue rail — say the gone-file recovery chain in filing.ts.
    else if (msg.includes("ALREADY_FILING"))
      toast("Ce morceau est déjà en cours de conversion.", false);
    else if (msg.toLowerCase().includes("upscale")) toast("Refusé : pas de surqualité lossy → lossless.", false);
    // Impasse A2 (issue #15). Cette branche passe AVANT le test de fichier introuvable, et l'ordre
    // est le correctif : `encode.rs` rend « ffmpeg: spawn failed: <erreur d'E/S> », et sur Windows
    // le message d'E/S est traduit par le système — « ... introuvable » en français. La branche
    // générique juste en dessous le matchait donc, et accusait le MORCEAU d'avoir été déplacé,
    // alors que c'est le binaire d'encodage qui manque. Le générique, lui, disait « Réessaie » à
    // une condition qui ne peut jamais aboutir : sans FFmpeg, aucun essai ne convertira jamais
    // rien. L'analyse passe par Symphonia in-process, donc l'utilisateur peut ajouter, scanner,
    // analyser et écouter avant de le découvrir — au premier « Ranger », c'est-à-dire ici.
    // Le littéral testé vient de notre propre code (`encode.rs`, EncodeError::Ffmpeg), pas d'un
    // message système : c'est ce qui le rend stable.
    else if (msg.includes("spawn failed"))
      toast(
        "FFmpeg est introuvable — Sift ne peut convertir aucun fichier tant qu'il manque. Réinstaller l'app le rétablit.",
        false,
      );
    else if (/permission|access|denied/i.test(msg)) toast("Refusé : accès au fichier/dossier refusé.", false);
    else if (/no such file|not found|introuvable/i.test(msg)) toast("Fichier introuvable — a-t-il été déplacé ?", false);
    else toast("La conversion a échoué. Le détail exact est dans la console.", false);
    console.error("file_track failed", e);
    setActionsDisabled(false);
    if (ranger && orig != null) ranger.innerHTML = orig;
  } finally {
    openState.acting = false;
  }
}

/** Show the filing confirmation as a BANNER at the TOP of the filing controls (`filedBannerHost`),
 *  above the next track's controls — the center has already auto-advanced to the next pending track (doRanger).
 *  This is the "after" proof for the file just filed: name + destination path + a targeted Revert.
 *  ONE banner at a time (replaces any prior). Revert is targeted on this file's `batchId`
 *  (revert_batch), available indefinitely via the journal; the ✕ dismisses the banner without
 *  reverting. Does NOT touch #mid or state.track — the advance owns those.
 *
 *  P5: it now opens in the "conversion en cours" state, because at this point nothing is converted
 *  yet — claiming "Converti" here would be a lie for as long as ffmpeg runs, and Annuler would
 *  target a journal batch that does not exist yet. `settleFilingBanner` flips it to Converti (and
 *  reveals Annuler) or to Échec when the backend reports. `data-batch-id` is how a late outcome
 *  recognises whether the banner still on screen is its own — a second conversion replaces it. */
function showFiledConfirm(
  trackId: number,
  batchId: string,
  bin: string,
  filedPath: string,
): void {
  state.filedConfirm = { batchId, bin };
  const foot = filedBannerHost();
  if (!foot) return;
  const filename = filedPath.split(/[\\/]/).pop() || filedPath;
  filedBannerEl()?.remove();
  const banner = document.createElement("div");
  banner.className = "sift-filed-banner";
  banner.dataset.batchId = batchId;
  banner.dataset.trackId = String(trackId);
  banner.setAttribute("role", "status");
  banner.setAttribute("aria-live", "polite");
  // Audit-ref (a11y, 2026-07-24): insert the (empty) live region into the document FIRST, then
  // populate it — a screen reader's mutation watcher only starts tracking a live region once it's
  // in the DOM; a node that arrives already fully populated in one mutation is inconsistently (or
  // never) announced by some ATs. filing-toast.ts's toast() has the same fill-then-append order —
  // left alone here since it's a separate, pre-existing site and not one of the confirmed findings.
  foot.prepend(banner);
  banner.innerHTML =
    `<div class="sift-filed-banner-head">` +
    `<i class="ti ti-loader-2 sift-spin" data-fil="filed-icon"></i>` +
    `<span class="sift-filed-banner-label" data-fil="filed-label">Conversion en cours…</span>` +
    `<span class="sift-filed-banner-bin">→ ${esc(bin)}</span>` +
    `<button data-fil="filed-close" title="Fermer" aria-label="Fermer" class="sift-filed-banner-close"><i class="ti ti-x"></i></button>` +
    `</div>` +
    `<div class="sift-filed-banner-name">${esc(filename)}</div>` +
    `<div class="sift-filed-banner-path">${esc(filedPath)}</div>` +
    // Hidden until the conversion actually commits: reverting a batch the journal doesn't hold yet
    // could only fail. Revealed by paintFiledBanner on success. Hidden by an INLINE display, not by
    // the `hidden` attribute: `.sift-filed-banner-revert` declares `display:inline-flex`, which
    // beats the UA stylesheet's `[hidden]{display:none}` and would leave the button visible.
    `<button data-fil="revert" class="sift-filed-banner-revert" style="display:none" hidden><i class="ti ti-arrow-back-up"></i> Annuler</button>`;
  banner.querySelector('[data-fil="revert"]')?.addEventListener("click", () => void doRevert(batchId));
  banner.querySelector('[data-fil="filed-close"]')?.addEventListener("click", () => {
    banner.remove();
    state.filedConfirm = null;
  });
  // The banner is only built AFTER the pane has advanced (openNext awaits several IPC round trips),
  // and a conformant file is merely tagged and moved — the conversion can therefore be over BEFORE
  // this node exists, in which case the outcome event found nothing to update. Paint the settled
  // state now rather than leave a spinner that would never stop. Painting the `running` case too
  // (rather than only the settled ones) is what strips the class's success-green tint off a banner
  // that announces work still in progress — the markup alone cannot do it.
  paintFiledBanner(
    banner,
    isFilingInFlight(trackId) ? "running" : filingFailure(trackId) ? "failed" : "done",
  );
}

/** The three visible states of the filing banner. `running` is the markup's own initial state. */
type FiledBannerState = "running" | "done" | "failed";

/** Move the banner between states by MUTATING the three nodes that change (icon, label, Annuler)
 *  rather than rewriting it — same discipline as the rest of the live UI, and it keeps the ✕
 *  handler alive. Annuler is only ever exposed in `done`: before that the journal holds no batch to
 *  revert, and after a failure there is nothing to undo. */
function paintFiledBanner(banner: HTMLElement, s: FiledBannerState): void {
  const failed = s === "failed";
  const running = s === "running";
  // `.sift-filed-banner` is now NEUTRAL by class (--color-background-secondary, 2026-08-24: no
  // permanent green aplat, components.md:193). Only `failed` re-tints, to the warning surface;
  // `running` and `done` both fall to the neutral class. The transient green lives in the flash
  // added at settle (settleFilingBanner), not here — announcing success in green while ffmpeg is
  // still encoding was exactly the lie P5 introduced. Inline, since these are states of an existing
  // component, not a new one (no new CSS rule).
  banner.style.background = failed ? "var(--color-background-warning)" : "";
  const neutral = failed ? "var(--color-text-warning)" : running ? "var(--color-text-tertiary)" : "";
  const icon = banner.querySelector<HTMLElement>('[data-fil="filed-icon"]');
  if (icon) {
    icon.className = running
      ? "ti ti-loader-2 sift-spin"
      : s === "done"
        ? "ti ti-check"
        : "ti ti-alert-triangle";
    icon.style.color = neutral;
  }
  const label = banner.querySelector<HTMLElement>('[data-fil="filed-label"]');
  if (label) {
    label.textContent = running
      ? "Conversion en cours…"
      : s === "done"
        ? "Converti"
        : "Conversion échouée";
    label.style.color = neutral;
  }
  const revert = banner.querySelector<HTMLElement>('[data-fil="revert"]');
  if (revert) {
    const shown = s === "done";
    revert.style.display = shown ? "" : "none";
    if (shown) revert.removeAttribute("hidden");
    else revert.setAttribute("hidden", "");
  }
}

/** Settle the on-screen banner when a background conversion reports (P5).
 *
 *  Only touches the banner when it is still THIS filing's (a second conversion has replaced it
 *  otherwise, and navigating away has removed it entirely). The toast is deliberately not the
 *  carrier of the information either: the durable signal is the queue-row marker driven by
 *  filing-state (D5 — the user is already elsewhere when a late failure lands). */
function settleFilingBanner(o: TrackFileOutcome, started: InFlightFiling | null): void {
  const banner = filedBannerEl();
  const mine = banner && banner.dataset.batchId === o.batch_id ? banner : null;
  if (o.error) {
    console.error("file_track background conversion failed", o.track_id, o.error);
    if (mine) {
      paintFiledBanner(mine, "failed");
      state.filedConfirm = null;
    }
    const name = started?.name ?? `#${o.track_id}`;
    toast(`Conversion échouée — ${name} est revenu dans la file`, false);
    return;
  }
  // Rangé avec succès : compté pour le fork empty-state (Tout est trié vs Rien à revoir). Compte les
  // filings Détail ; le lot incrémentera de même en Lane 3 (onFileBatchDone, batch-panel.ts).
  state.filedThisSession++;
  if (mine) {
    paintFiledBanner(mine, "done");
    // Flash vert bref au rangement (DESIGN §8 : un état permanent reste neutre, seule la transition
    // se colore). Réutilise .sift-identified-flash, retiré à animationend (comme filing-identify.ts).
    mine.classList.add("sift-identified-flash");
    mine.addEventListener("animationend", () => mine.classList.remove("sift-identified-flash"), {
      once: true,
    });
  }
}

// Registered at module load (pure bookkeeping — the Tauri subscription itself is installed lazily
// by ensureFilingWatcher, from doRanger). One registration for the whole session: the banner it
// updates is looked up at event time, never captured.
onFilingOutcome(settleFilingBanner);

/** Revert THIS file's filing, targeted on its `batchId` (revert_batch). On success the engine
 *  puts the track back to pending and emits queue:changed → the queue refreshes. On a Blocked
 *  engine error (e.g. the original was purged from the trash) show a clear message rather than
 *  failing mutely. The revert engine itself is untouched here. */
async function doRevert(batchId: string): Promise<void> {
  try {
    await revertBatch(batchId);
    filedBannerEl()?.remove();
    state.filedConfirm = null;
    toast("Annulé — retour dans la file", false);
  } catch (e) {
    // Le message de domaine passe en `display` : les DEUX branches traversent donc
    // `humanizeError`, et aucune ne reste sans trace. Une version antérieure de ce correctif
    // n'appelait le module que dans le `else` tout en retirant le `console.error` qui couvrait les
    // deux — la branche « source gone », la plus documentée de ce chemin, n'écrivait plus rien.
    const msg = String(e);
    toast(
      humanizeError(
        e,
        msg.includes("source gone")
          ? "Annulation impossible : un fichier nécessaire a disparu — l'original a peut-être été purgé de la corbeille."
          : "Échec de l'annulation — réessaie",
        "revert",
      ),
      false,
    );
  }
}

/** Re-sourcer (fake) ou Écarter (non-fake) the current track — both are the same reversible
 *  reject_track path now (annotation: "jeter devrait etre écarté, et finir dans écarter"); `kind`
 *  stays two-valued only to pick the right toast wording, not a different backend action anymore. */
export async function doSecondary(
  mid: HTMLElement,
  kind: "resource" | "trash",
  clearPane: (mid: HTMLElement, emptyQueue?: boolean) => void,
): Promise<void> {
  if (!state.track || openState.acting) return;
  const trackId = state.track.id;
  openState.acting = true;
  setActionsDisabled(true);
  try {
    await rejectTrack(trackId);
    toast(kind === "resource" ? "Marqué à re-sourcer" : "Écarté", true, () => {
      void requeueTrack(trackId).catch((e) => {
        toast(humanizeError(e, "Échec de l'annulation — réessaie", `${kind} undo`), false);
      });
    });
    clearPane(mid);
  } catch (e) {
    toast(
      humanizeError(
        e,
        kind === "resource"
          ? "Impossible de marquer à re-sourcer — réessaie"
          : "Impossible d'écarter la piste — réessaie",
        kind,
      ),
      false,
    );
    setActionsDisabled(false);
  } finally {
    openState.acting = false;
  }
}
