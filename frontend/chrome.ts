// App chrome (Tauri only): custom titlebar, the "lean" stylesheet that hides the mockup's
// not-yet-real surfaces, scroll-thumb autohide, and OS drag-drop. Extracted from sift-live.ts
// (audit P-3) — self-contained UI shell, no shared app state; imports only Tauri + ipc.
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { importPaths } from "./ipc";

// One-time style: while dragging, an existing zone gets an outline + an overlaid hint
// (::after with the zone's data-dz text). No permanent dashed box — the hint shows only
// during a drag, on the real folder/queue boxes, saving space.
function ensureDropStyle() {
  if (document.getElementById("sift-dz-style")) return;
  const s = document.createElement("style");
  s.id = "sift-dz-style";
  s.textContent =
    ".sift-dz-on{position:relative;outline:1.5px dashed var(--color-text-info);outline-offset:-4px;border-radius:var(--border-radius-md)}" +
    ".sift-dz-on::after{content:attr(data-dz);position:absolute;inset:0;display:flex;align-items:center;justify-content:center;text-align:center;padding:10px;font-size:var(--text-sm);color:var(--color-text-info);background:rgba(20,20,24,.55);border-radius:var(--border-radius-md);pointer-events:none;z-index:50}";
  document.head.appendChild(s);
}

// Existing boxes that double as drop targets, with the hint each shows while dragging.
// "#filfoot" is the action rail carrying the Destination button (the tree itself is a popover,
// hidden by default, so it can't be a reliable drop target) — a folder dropped on the rail
// registers as the new destination.
const DROP_ZONES: [string, string][] = [
  ["#filfoot", "Dépose un dossier ici — nouvelle destination"],
  ["#ql", "Dépose des fichiers audio ici"],
  ["#sift-sources", "Dépose un dossier à surveiller"],
];

/** Toggle the drag hint/outline on the relevant existing boxes. Falls back to #content
 * (e.g. Bibliothèque) when none of the named zones are on screen. */
function setDropActive(on: boolean) {
  ensureDropStyle();
  document.querySelectorAll<HTMLElement>(".sift-dz-on").forEach((el) => {
    el.classList.remove("sift-dz-on");
    el.removeAttribute("data-dz");
  });
  if (!on) return;
  const present = DROP_ZONES.filter(([sel]) => document.querySelector(sel));
  const targets: [string, string][] = present.length
    ? present
    : [["#content", "Dépose des fichiers (→ file d'attente) ou des dossiers (→ surveillés)"]];
  for (const [sel, label] of targets) {
    const el = document.querySelector<HTMLElement>(sel);
    if (el) {
      el.classList.add("sift-dz-on");
      el.dataset.dz = label;
    }
  }
}

/** "dest" when the cursor is over the bins column (#fldz), else "source". Tauri 2 emits the
 * drop position already in logical (CSS) pixels — exactly what elementFromPoint expects, so
 * no devicePixelRatio correction (dividing here double-corrected on HiDPI/scaled displays). */
function dropModeAt(pos: { x: number; y: number }): "source" | "dest" {
  const el = document.elementFromPoint(pos.x, pos.y);
  return el && el.closest("#filfoot") ? "dest" : "source";
}

/** OS drag-drop: audio files → queue; folders → watched source, or a destination bin when
 * dropped on the "Où on va" column. */
export async function installDragDrop() {
  try {
    await getCurrentWebview().onDragDropEvent((ev) => {
      const p = ev.payload;
      if (p.type === "drop") {
        setDropActive(false);
        if (p.paths.length)
          void importPaths(p.paths, dropModeAt(p.position)).catch((e) =>
            console.error("import_paths failed", e),
          );
      } else if (p.type === "enter" || p.type === "over") {
        setDropActive(true);
      } else {
        setDropActive(false);
      }
    });
  } catch (e) {
    console.error("drag-drop init failed", e);
  }
}

/** Lean Tauri UI: hide the mockup's not-yet-real surfaces (nav tabs + Revue toggles) so the
 * app shows only what actually works — Accueil (sources) and Revue (queue/report/filing).
 * Injected once; the demo (plain browser) never runs this, so its full mockup is untouched. */
export function injectLeanStyle() {
  if (document.getElementById("sift-lean-style")) return;
  const st = document.createElement("style");
  st.id = "sift-lean-style";
  st.textContent =
    // landing/demo copy in index.html: marketing pitch, demo disclaimer, feature cards row
    ".pitch,.sub,.frow{display:none!important}" +
    // Rekordbox/Clé USB nav items: wired to a real (simulated) export task in sift-live.ts
    // (runNavExport) as of 2026-07-02 — no longer hidden. Revue-only toggles below still are.
    // Revue: batch mode + "traités" toggle aren't wired to the real backend yet
    '[data-act="revmode"],[data-act="togglequeue"]{display:none!important}' +
    // custom frameless titlebar (decorations are off in tauri.conf — Tauri only)
    "#sift-titlebar{height:30px;flex:none;display:flex;align-items:center;justify-content:space-between;" +
    "background:var(--color-background-tertiary);-webkit-user-select:none;user-select:none}" +
    "#sift-tb-title{padding-left:13px;font-size:var(--text-sm);letter-spacing:.04em;color:var(--color-text-tertiary)}" +
    "#sift-tb-controls{display:flex;height:100%}" +
    ".sift-win{width:44px;height:100%;display:flex;align-items:center;justify-content:center;border:none;" +
    "background:transparent;color:var(--color-text-tertiary);cursor:pointer;border-radius:0;padding:0}" +
    ".sift-win:hover{background:var(--color-background-secondary);color:var(--color-text-primary)}" +
    ".sift-win-close:hover{background:#e81123;color:#fff}.sift-win i{font-size:15px}" +
    // make room for the 30px bar: shrink the app shell so nothing is clipped
    "#pa{height:calc(100vh - 30px)!important}";
  document.head.appendChild(st);
}

/** Inject the custom window titlebar (the native one is off via decorations:false) and wire
 * its minimise / maximise / close buttons. The bar + its title are drag regions. */
export function injectTitlebar() {
  if (document.getElementById("sift-titlebar")) return;
  const bar = document.createElement("div");
  bar.id = "sift-titlebar";
  bar.setAttribute("data-tauri-drag-region", "");
  bar.innerHTML =
    '<span id="sift-tb-title" data-tauri-drag-region>Sift</span>' +
    '<div id="sift-tb-controls">' +
    '<button class="sift-win" data-win="min" title="Réduire"><i class="ti ti-minus"></i></button>' +
    '<button class="sift-win" data-win="max" title="Agrandir"><i class="ti ti-square"></i></button>' +
    '<button class="sift-win sift-win-close" data-win="close" title="Fermer"><i class="ti ti-x"></i></button>' +
    "</div>";
  document.body.insertBefore(bar, document.body.firstChild);

  const w = getCurrentWindow();
  bar.querySelectorAll<HTMLElement>(".sift-win").forEach((b) =>
    b.addEventListener("click", () => {
      const act = b.dataset.win;
      if (act === "min") void w.minimize();
      else if (act === "max") void w.toggleMaximize();
      else if (act === "close") void w.close();
    }),
  );
}

/** Reveal a scroll area's thumb while it scrolls, then hide it ~700ms after it stops (the
 * CSS keeps it hidden at rest). Capture-phase so it catches scrolling on any inner element. */
export function installScrollAutohide() {
  const timers = new WeakMap<Element, ReturnType<typeof setTimeout>>();
  document.addEventListener(
    "scroll",
    (e) => {
      const el = e.target;
      if (!(el instanceof Element)) return;
      el.classList.add("sift-scrolling");
      const prev = timers.get(el);
      if (prev) clearTimeout(prev);
      timers.set(
        el,
        setTimeout(() => el.classList.remove("sift-scrolling"), 700),
      );
    },
    true,
  );
}
