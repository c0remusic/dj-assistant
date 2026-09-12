// Generic in-app confirmation overlay — replaces window.confirm() everywhere in Sift. See
// CLAUDE.md: a real incident happened when a synthetic click ran straight through window.confirm()
// in this Tauri/WebView2 setup with no dialog ever appearing, filing 265 tracks by accident. A
// real DOM button, like every other control in this app, doesn't have that blocking-OS-dialog
// bypass — the returned promise only resolves on an actual click landing inside the webview.
// Lighter than usb-format-modal.ts's typed+armed cycle: that extra friction is reserved for the
// one truly irreversible action (disk format) — everything else stays at today's single-confirm
// friction level, just delivered reliably.
const OVERLAY_ID = "sift-confirm-overlay";

/** Above how many tracks a destructive mass action asks a second time.
 *
 *  Déclarée ICI et non chez son premier appelant : deux écrans posent désormais la même question
 *  — le mode Lot de Revue (`batch-panel.ts`, qui l'arme dans son propre bouton) et les actions de
 *  masse de la Bibliothèque (`bibliotheque-view.ts`, qui passe par `confirmAction`). Deux copies
 *  du nombre laisseraient les deux écrans diverger sans que rien ne le signale.
 *
 *  Le seuil ne vise pas la réversibilité — écarter et corbeille sont l'un et l'autre annulables —
 *  il vise le clic qui n'est pas humain, et `⌘/Ctrl+A` sur une liste virtualisée sélectionne
 *  précisément ce que l'utilisateur ne voit pas. */
export const BATCH_CONFIRM_THRESHOLD = 10;

/** How long the destructive button stays genuinely `disabled` after the overlay opens.
 *  Single source of truth: read by the `disabled` timer AND by the timestamped guard in the
 *  confirm handler, so the two can never disagree about the window they enforce. */
const CONFIRM_ARM_MS = 250;

/** Remove `sift-fade-from` on the SECOND animation frame, not the first.
 *  A rAF callback runs BEFORE style recalc in Chromium/WebView2, so a class removed there means
 *  the node never held a computed `opacity:0` — there is nothing to transition from, and the fade
 *  is silently skipped (it looks "shipped" while never having played, and nothing automated
 *  catches it). The second frame is after the recalc, so the from-state is real. */
function playFadeIn(el: HTMLElement): void {
  el.classList.add("sift-fade-in", "sift-fade-from");
  requestAnimationFrame(() => requestAnimationFrame(() => el.classList.remove("sift-fade-from")));
}

// The previous call's `finish`, if its overlay is still open (promise unresolved) — set/cleared
// by confirmAction below. Audit 2026-07-10: without settling it first, a second confirmAction()
// call only removed the first overlay's DOM node (see below) but left its `keydown` listener
// attached forever, so its Tab-focus-trap kept hijacking every Tab keypress app-wide afterwards.
let activeFinish: ((result: boolean) => void) | null = null;

export interface BatchAlertData {
  fileCount: number;
  fakeCount: number;
  destLabel: string;
  formatSummary: string;
}

export interface BatchAlertResult {
  confirmed: boolean;
  skipFuture: boolean;
}

export function confirmBatchAlert(data: BatchAlertData): Promise<BatchAlertResult> {
  activeFinish?.(false);
  document.getElementById(OVERLAY_ID)?.remove();
  const previouslyFocused = document.activeElement as HTMLElement | null;
  return new Promise((resolve) => {
    let skipChecked = false;

    const overlay = document.createElement("div");
    overlay.id = OVERLAY_ID;
    overlay.className = "sift-report-overlay";

    const card = document.createElement("div");
    card.className = "sift-report-overlay-card sift-confirm-card sift-batch-alert";
    card.setAttribute("role", "alertdialog");
    card.setAttribute("aria-modal", "true");
    card.setAttribute("aria-label", "Ranger la sélection ?");

    const title = document.createElement("div");
    title.className = "sift-batch-alert-title";
    title.textContent = "Ranger la sélection ?";

    const recap = document.createElement("div");
    recap.className = "sift-batch-alert-recap";
    const parts: string[] = [];
    parts.push(`${data.fileCount} prête${data.fileCount > 1 ? "s" : ""} → Convertir`);
    if (data.fakeCount > 0) {
      parts.push(`${data.fakeCount} FAKE → Écarter`);
    }
    recap.textContent = parts.join(" · ");

    const destLine = document.createElement("div");
    destLine.className = "sift-batch-alert-dest";
    destLine.textContent = `Destination · ${data.destLabel} · ${data.formatSummary}`;

    const checkLabel = document.createElement("label");
    checkLabel.className = "sift-batch-alert-skip";
    const checkInput = document.createElement("input");
    checkInput.type = "checkbox";
    checkInput.addEventListener("change", () => {
      skipChecked = checkInput.checked;
    });
    const checkText = document.createTextNode(" Ne plus me demander (cette session)");
    checkLabel.append(checkInput, checkText);

    const actions = document.createElement("div");
    actions.className = "sift-confirm-actions";
    const cancelBtn = document.createElement("button");
    cancelBtn.type = "button";
    cancelBtn.className = "sift-settings-btn";
    cancelBtn.textContent = "Annuler";
    const confirmBtn = document.createElement("button");
    confirmBtn.type = "button";
    confirmBtn.className = "sift-confirm-btn";
    confirmBtn.textContent = `Ranger ${data.fileCount}`;
    confirmBtn.disabled = true;
    const armTimer = window.setTimeout(() => {
      confirmBtn.disabled = false;
    }, CONFIRM_ARM_MS);
    const openedAt = Date.now();
    actions.append(cancelBtn, confirmBtn);

    card.append(title, recap, destLine, checkLabel, actions);
    overlay.appendChild(card);
    document.body.appendChild(overlay);
    playFadeIn(overlay);
    cancelBtn.focus();

    const finish = (confirmed: boolean) => {
      document.removeEventListener("keydown", onKeydown);
      clearTimeout(armTimer);
      overlay.remove();
      previouslyFocused?.focus();
      activeFinish = null;
      resolve({ confirmed, skipFuture: confirmed && skipChecked });
    };
    activeFinish = (result: boolean) => finish(result);
    const onKeydown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        finish(false);
        return;
      }
      if (e.key === "Tab") {
        e.preventDefault();
        const focusable: HTMLElement[] = [cancelBtn, checkInput, confirmBtn];
        const idx = focusable.indexOf(document.activeElement as HTMLElement);
        const next = e.shiftKey
          ? (idx <= 0 ? focusable.length - 1 : idx - 1)
          : idx >= focusable.length - 1
            ? 0
            : idx + 1;
        (focusable[next] as HTMLElement).focus();
      }
    };
    document.addEventListener("keydown", onKeydown);
    cancelBtn.addEventListener("click", () => finish(false));
    confirmBtn.addEventListener("click", () => {
      if (Date.now() - openedAt < CONFIRM_ARM_MS) return;
      finish(true);
    });
    overlay.addEventListener("click", (e) => {
      if (e.target === overlay) finish(false);
    });
  });
}

export function confirmAction(message: string, confirmLabel = "Confirmer"): Promise<boolean> {
  // Settle any still-open prior call first — removes its keydown listener and resolves its
  // promise, instead of leaking both when this new call replaces its overlay below.
  activeFinish?.(false);
  document.getElementById(OVERLAY_ID)?.remove();
  const previouslyFocused = document.activeElement as HTMLElement | null;
  return new Promise((resolve) => {
    const overlay = document.createElement("div");
    overlay.id = OVERLAY_ID;
    overlay.className = "sift-report-overlay";

    const card = document.createElement("div");
    // `sift-report-overlay-card-blur` retirée le 2026-08-19 avec sa règle CSS : c'était la dernière
    // fausse translucidité de l'app (color-mix + backdrop-filter + !important). La carte retombe sur
    // l'aplat opaque de `.sift-report-overlay-card`. Voir styles.css, au-dessus de cette règle.
    card.className = "sift-report-overlay-card sift-confirm-card";
    // Audit-ref R5 (Revue, 2026-07-08, réf. shadcn Alert Dialog) : ni sémantique modale ni Escape
    // avant ce fix — seul le clic sur le fond annulait. Cette modale gère des actions destructives
    // (règle CLAUDE.md anti-window.confirm()), donc le clavier doit marcher comme partout ailleurs.
    card.setAttribute("role", "alertdialog");
    card.setAttribute("aria-modal", "true");
    card.setAttribute("aria-label", message);

    const msg = document.createElement("div");
    msg.className = "sift-confirm-msg";
    msg.textContent = message;

    const actions = document.createElement("div");
    actions.className = "sift-confirm-actions";
    const cancelBtn = document.createElement("button");
    cancelBtn.type = "button";
    cancelBtn.className = "sift-settings-btn";
    cancelBtn.textContent = "Annuler";
    const confirmBtn = document.createElement("button");
    confirmBtn.type = "button";
    confirmBtn.className = "sift-confirm-btn";
    confirmBtn.textContent = confirmLabel;
    // Disarm the destructive button for CONFIRM_ARM_MS, via `disabled` — NOT via
    // `pointer-events:none`, which stops real pointer input but does NOT stop
    // HTMLElement.click(). A synthetic click is precisely the failure mode that made this module
    // exist (see the header), so the only acceptable guard is the one the platform enforces on the
    // dispatch itself. `openedAt` below backs it up in the handler.
    confirmBtn.disabled = true;
    const armTimer = window.setTimeout(() => {
      confirmBtn.disabled = false;
    }, CONFIRM_ARM_MS);
    const openedAt = Date.now();
    actions.append(cancelBtn, confirmBtn);

    card.append(msg, actions);
    overlay.appendChild(card);
    document.body.appendChild(overlay);
    // One 150ms fade on the way IN, on the overlay only. Nothing is animated on the way OUT: an
    // exit fade would keep a dismissed destructive prompt on screen — and clickable — after the
    // user has already answered it.
    playFadeIn(overlay);
    // Audit 2026-07-09 : focaliser confirmBtn par défaut expose à valider une action destructrice
    // (dedup, réparations master.db) sur un Entrée/Espace résiduel juste après ouverture — même
    // logique que shadcn Alert Dialog (focus par défaut sur Cancel), déjà notre référence pour
    // Escape/role ci-dessus.
    cancelBtn.focus();

    const finish = (result: boolean) => {
      document.removeEventListener("keydown", onKeydown);
      clearTimeout(armTimer);
      overlay.remove();
      // Restore focus to whatever opened the modal — without this, focus falls back to <body>,
      // disorienting for keyboard/screen-reader users after a destructive-action prompt closes.
      previouslyFocused?.focus();
      activeFinish = null;
      resolve(result);
    };
    activeFinish = finish;
    const onKeydown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        finish(false); // R5 : Escape annule, comme shadcn Alert Dialog
        return;
      }
      // Focus trap: this modal has exactly 2 focusable elements — cycle Tab between them so
      // keyboard focus can never land on the app behind the overlay while it's open.
      if (e.key === "Tab") {
        e.preventDefault();
        if (document.activeElement === cancelBtn) confirmBtn.focus();
        else cancelBtn.focus();
      }
    };
    document.addEventListener("keydown", onKeydown);
    cancelBtn.addEventListener("click", () => finish(false));
    // Timestamped guard on the same window, modelled on batch-panel.ts's armed-confirm floor
    // (`Date.now() - batchConfirmArmed.at >= 400`). `disabled` above is the primary guard; this is
    // the belt to its braces, evaluated at click time, and it holds even if the button were
    // re-enabled early by some future edit or if a click were dispatched at the exact boundary.
    confirmBtn.addEventListener("click", () => {
      if (Date.now() - openedAt < CONFIRM_ARM_MS) return;
      finish(true);
    });
    overlay.addEventListener("click", (e) => {
      if (e.target === overlay) finish(false);
    });
  });
}
