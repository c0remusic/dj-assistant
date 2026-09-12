import type { Target } from "../shared/contracts";
import { previewFilename } from "./ipc";
import { state } from "./filing-state";

/** Capitalise the first letter of each word ("original mix" → "Original Mix"), leaving the rest
 *  as-is so existing caps/acronyms ("2WFU Dub", "Knee Deep Remix") survive. */
const titleCase = (s: string): string =>
  s.replace(/(^|[\s(/-])([\p{L}\p{N}])/gu, (_, sep: string, ch: string) => sep + ch.toUpperCase());

export const TARGET_LABEL: Record<Target, string> = {
  mp3_320: "MP3",
  aiff_16_44: "AIFF",
  wav_16_44: "WAV",
};

/** Default target from the analysed rail (lossless → AIFF, else MP3 320). */
function defaultTarget(rail: string): Target {
  return rail === "lossless" ? "aiff_16_44" : "mp3_320";
}

function targetExt(t: Target): string {
  if (t === "mp3_320") return "mp3";
  if (t === "wav_16_44") return "wav";
  return "aiff";
}

/** Clean display name from the (edited) canonical — what the file will be called. */
function displayName(): string {
  const c = state.canonical;
  if (!c) return "";
  const ver = c.version && c.version.trim() ? ` (${c.version.trim()})` : "";
  return c.artist ? `${c.artist} — ${c.title}${ver}` : `${c.title}${ver}`;
}

/** Cross-fades a text swap instead of an abrupt content jump (annotation: ".sift-player-name
 * semble afficher une valeur differente... pendant une micro seconde, ça n'est pas fluide") —
 * the header paints the raw filename synchronously (report-view.ts playerRowHtml, so something
 * shows instantly), then this fn overwrites it with the reconciled title once reconcile()/
 * trackRelease() resolve a moment later. No-ops if the value hasn't actually changed, so the
 * common already-correct re-render (e.g. plain edits) never fades for no reason. */
function fadeSetText(el: HTMLElement, next: string): void {
  if (el.textContent === next) return;
  if (el.classList.contains("sift-report-text-pending")) {
    el.textContent = next;
    el.classList.remove("sift-report-text-pending");
    el.style.opacity = "";
    el.style.transition = "";
    return;
  }
  el.style.transition = "opacity .1s ease";
  el.style.opacity = "0";
  window.setTimeout(() => {
    el.textContent = next;
    el.style.opacity = "1";
  }, 100);
}

/** Replace the report header's filename with the clean proposed name (raw path stays as the
 * grey subtitle), so a messy source file shows its tidy target name. */
function updateHeaderName(mid: HTMLElement): void {
  const c = state.canonical;
  if (!c) return; // before reconcile: keep the filename the report set
  // Two copies live in the DOM at once now (the big Hero + the player's mini header, same
  // classes) — update every match, not just the first, so the second copy doesn't go stale.
  // "Any matches at all?" is still a normal question (mid may have been replaced after an
  // await / a navigation) → probe non-throw and no-op on an empty NodeList, like renderQueue's
  // `if (!ql) return`.
  const ver = c.version && c.version.trim() ? c.version.trim() : "";
  mid.querySelectorAll<HTMLElement>(".sift-report-name").forEach((el) => {
    // Board hero: big TITLE on top, "artist · version" subtitle below (not the full filename).
    fadeSetText(el, c.title || displayName());
  });
  mid.querySelectorAll<HTMLElement>(".sift-report-sub").forEach((el) => {
    fadeSetText(el, [c.artist, ver].filter(Boolean).join(" · "));
  });
}

// FIX-12: refreshPreview is wired to the artist/title/version inputs' `input` event — fires on
// every keystroke. previewFilename() is an IPC round-trip, so debounce it (150ms), and guard
// with a sequence token so a slow/reordered response from an earlier keystroke can never
// overwrite the result of a newer one (same hazard class as identify/openFilingInto elsewhere).
let previewSeq = 0;
let previewTimer: ReturnType<typeof setTimeout> | undefined;

/** Re-sync the filename preview from the current canonical + target. The preview lives in the
 *  rangée réglages de la BOÎTE de lecture (`#filbox-settings`, renderFoot) depuis la décision V2b
 *  du 2026-08-30 — elle était dans le pied de panneau (#filfoot) avant. Elle avait quitté la
 *  conclusion de verdict au redesign du 2026-07-06 (cette carte est la CONCLUSION, pas le lieu du
 *  nom final). A format change or a field edit must refresh this node (the extension follows
 *  state.target). La sonde reste document-wide et non-throw : le nœud peut avoir disparu, et sa
 *  position dans le DOM ne concerne pas cette fonction. Renders via naming::render_filename (real
 *  template + sanitize()) in Rust — not a TS reimplementation. */
function refreshPreview(): void {
  const c = state.canonical;
  const prev = document.querySelector<HTMLElement>(".sift-fil-prev");
  if (!c) {
    if (prev) prev.textContent = "";
    return;
  }
  // Same default as the lit format chip (renderFoot): state.target when set, else defaultTarget(rail).
  const ext = targetExt(state.target ?? defaultTarget(state.rail));
  const mySeq = ++previewSeq;
  if (previewTimer !== undefined) clearTimeout(previewTimer);
  previewTimer = setTimeout(() => {
    void previewFilename(c, ext)
      .then((name) => {
        if (mySeq !== previewSeq) return; // superseded by a newer edit — drop this stale result
        if (prev) prev.textContent = `→ ${name}`;
      })
      .catch((e) => console.error("previewFilename failed", e));
  }, 150);
}

export { titleCase, defaultTarget, displayName, fadeSetText, updateHeaderName, refreshPreview };
