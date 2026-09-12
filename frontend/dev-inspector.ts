// Dev-only click-to-source inspector: Alt+Click any element in the running app to see
// which real file(s)/line(s) define or consume its class/id — for pointing Claude at an
// exact spot instead of describing it. Never active outside `import.meta.env.DEV` (see
// main.ts) and the backend command it calls (locate_source) refuses outside debug builds
// too (see src-tauri/src/dev_locate.rs) — belt and suspenders against shipping this.
//
// Alt+Click accumulates a selection (repeat Alt+Click on other elements adds them; Alt+Click
// an already-selected element removes it) so one note + Envoyer can carry several elements at
// once — needed when the problem is a relationship between two zones ("pas cohérent avec la
// Bibliothèque"), not a single element in isolation.
import { invoke } from "@tauri-apps/api/core";
import { buildAnnotation, sendAnnotation } from "./dev-annotate";

interface SourceMatch {
  file: string;
  line: number;
  excerpt: string;
}

interface Selected {
  el: HTMLElement;
  box: HTMLElement;
}

function describe(el: HTMLElement): string {
  const id = el.id ? `#${el.id}` : "";
  const cls = el.classList.length ? `.${[...el.classList].join(".")}` : "";
  return `<${el.tagName.toLowerCase()}>${id}${cls}`;
}

function buildHighlight(el: HTMLElement): HTMLElement {
  const box = document.createElement("div");
  box.style.cssText =
    "position:fixed;pointer-events:none;z-index:99998;" +
    "border:2px solid #f2c274;border-radius:3px;box-shadow:0 0 0 2px rgba(0,0,0,.35)";
  document.body.appendChild(box);
  moveHighlight(box, el);
  return box;
}

function moveHighlight(box: HTMLElement, el: HTMLElement) {
  const r = el.getBoundingClientRect();
  box.style.left = `${r.left - 2}px`;
  box.style.top = `${r.top - 2}px`;
  box.style.width = `${r.width}px`;
  box.style.height = `${r.height}px`;
}

export function installDevInspector() {
  // État de la sélection multi, persistant tant que le panneau reste ouvert.
  let panel: HTMLElement | null = null;
  let selListZone: HTMLElement;
  let locateZone: HTMLElement;
  let note: HTMLTextAreaElement;
  let sendBtn: HTMLButtonElement;
  let status: HTMLElement;
  const selection: Selected[] = [];

  function teardown() {
    panel?.remove();
    for (const s of selection) s.box.remove();
    selection.length = 0;
    panel = null;
  }

  function renderSelectionList() {
    selListZone.replaceChildren();
    if (selection.length === 0) {
      const none = document.createElement("div");
      none.style.color = "#847E75";
      none.textContent = "aucun élément sélectionné — Alt+Clic pour en ajouter";
      selListZone.appendChild(none);
      return;
    }
    selection.forEach((s, i) => {
      const row = document.createElement("div");
      row.style.cssText = "display:flex;align-items:center;gap:6px;margin:2px 0;color:#9fe0af";
      const label = document.createElement("span");
      label.textContent = `${i + 1}. ${describe(s.el)}`;
      label.style.cssText = "flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap";
      const rm = document.createElement("button");
      rm.textContent = "×";
      rm.title = "retirer de la sélection";
      rm.style.cssText = "cursor:pointer;color:#f2c274";
      rm.onclick = () => {
        s.box.remove();
        selection.splice(i, 1);
        renderSelectionList();
        refreshLocateButtons();
      };
      row.appendChild(label);
      row.appendChild(rm);
      selListZone.appendChild(row);
    });
  }

  function refreshLocateButtons() {
    locateZone.replaceChildren();
    const active = selection[selection.length - 1]?.el;
    if (!active) return;
    const heading = document.createElement("div");
    heading.style.cssText = "color:#847E75;margin-bottom:2px";
    heading.textContent = `localiser (dernier ajouté : ${describe(active)})`;
    locateZone.appendChild(heading);
    const identifiers = active.id ? [`#${active.id}`, ...active.classList] : [...active.classList];
    if (identifiers.length === 0) {
      const none = document.createElement("div");
      none.textContent = "pas de classe/id — capture par contexte seulement";
      locateZone.appendChild(none);
      return;
    }
    const resultZone = document.createElement("div");
    for (const id of identifiers) {
      const btn = document.createElement("button");
      btn.textContent = id;
      btn.style.cssText = "margin:2px;cursor:pointer";
      btn.onclick = () => void showMatches(resultZone, id);
      locateZone.appendChild(btn);
    }
    locateZone.appendChild(resultZone);
  }

  async function showMatches(zone: HTMLElement, identifier: string) {
    const heading = document.createElement("div");
    heading.textContent = `→ ${identifier}`;
    heading.style.cssText = "margin-top:8px;color:#9fe0af;font-weight:bold";
    zone.appendChild(heading);
    try {
      const matches = await invoke<SourceMatch[]>("locate_source", { identifier });
      if (matches.length === 0) {
        const none = document.createElement("div");
        none.textContent = "(aucun résultat dans frontend/)";
        zone.appendChild(none);
        return;
      }
      for (const m of matches.slice(0, 8)) {
        const row = document.createElement("div");
        row.style.cssText = "margin:4px 0;white-space:pre-wrap;color:#ccc";
        row.textContent = `${m.file}:${m.line}\n${m.excerpt}`;
        zone.appendChild(row);
      }
      if (matches.length > 8) {
        const more = document.createElement("div");
        more.style.color = "#847E75";
        more.textContent = `… ${matches.length - 8} de plus, non affichés`;
        zone.appendChild(more);
      }
    } catch (e) {
      const err = document.createElement("div");
      err.style.color = "#f2c274";
      err.textContent = String(e);
      zone.appendChild(err);
    }
  }

  function buildPanel(): HTMLElement {
    const p = document.createElement("div");
    p.id = "sift-dev-inspector";
    p.style.cssText =
      "position:fixed;top:8px;right:8px;width:440px;max-height:75vh;overflow-y:auto;" +
      "background:#1e1e1e;color:#eee;font:11px 'JetBrains Mono',monospace;padding:10px;" +
      "border-radius:6px;z-index:99999;box-shadow:0 8px 24px rgba(0,0,0,.5)";
    document.body.appendChild(p);

    const closeBtn = document.createElement("button");
    closeBtn.textContent = "× fermer";
    closeBtn.style.cssText = "float:right;cursor:pointer;margin-bottom:6px";
    closeBtn.onclick = teardown;
    p.appendChild(closeBtn);

    const header = document.createElement("div");
    header.style.cssText = "color:#9fe0af;font-weight:bold;margin-bottom:6px";
    header.textContent = "Sélection (Alt+Clic pour ajouter/retirer)";
    p.appendChild(header);

    selListZone = document.createElement("div");
    selListZone.style.cssText = "margin-bottom:8px";
    p.appendChild(selListZone);

    const parentBtn = document.createElement("button");
    parentBtn.textContent = "⬆ bloc parent (dernier ajouté)";
    parentBtn.style.cssText = "cursor:pointer;margin-bottom:6px";
    parentBtn.onclick = () => {
      const last = selection[selection.length - 1];
      if (!last) return;
      const parentEl = last.el.parentElement;
      if (!parentEl || parentEl === document.body) return;
      last.el = parentEl;
      moveHighlight(last.box, parentEl);
      renderSelectionList();
      refreshLocateButtons();
    };
    p.appendChild(parentBtn);

    note = document.createElement("textarea");
    note.placeholder = "Remarque libre (« trop tassé », « pas cohérent avec la Bibliothèque »...)";
    note.style.cssText =
      "width:100%;min-height:56px;box-sizing:border-box;margin-bottom:4px;" +
      "background:#2a2a2a;color:#eee;border:1px solid #555;border-radius:4px;padding:6px;font:inherit";
    p.appendChild(note);

    sendBtn = document.createElement("button");
    sendBtn.textContent = "Envoyer";
    sendBtn.style.cssText = "cursor:pointer;margin-bottom:8px";
    sendBtn.onclick = () => {
      const text = note.value.trim();
      if (selection.length === 0) { status.textContent = "sélectionne au moins un élément (Alt+Clic)"; return; }
      if (!text) { status.textContent = "note vide — écris d'abord ta remarque"; return; }
      sendBtn.disabled = true;
      status.textContent = "envoi…";
      void buildAnnotation(selection.map((s) => s.el), text)
        .then(sendAnnotation)
        .then(() => {
          status.textContent = "✓ envoyée (docs/annotations.jsonl)";
          note.value = "";
          for (const s of selection) s.box.remove();
          selection.length = 0;
          renderSelectionList();
          refreshLocateButtons();
        })
        .catch((err) => { status.textContent = `échec : ${String(err)}`; })
        .finally(() => { sendBtn.disabled = false; });
    };
    p.appendChild(sendBtn);

    status = document.createElement("div");
    status.style.cssText = "margin-bottom:8px;color:#847E75";
    p.appendChild(status);

    locateZone = document.createElement("div");
    p.appendChild(locateZone);

    return p;
  }

  document.addEventListener(
    "click",
    (e) => {
      if (!e.altKey) return;
      e.preventDefault();
      e.stopPropagation();

      const el = e.target as HTMLElement;
      if (!panel) panel = buildPanel();

      const existingIdx = selection.findIndex((s) => s.el === el);
      if (existingIdx >= 0) {
        selection[existingIdx].box.remove();
        selection.splice(existingIdx, 1);
      } else {
        selection.push({ el, box: buildHighlight(el) });
      }
      renderSelectionList();
      refreshLocateButtons();
    },
    true,
  );
  console.log("Sift dev inspector: Alt+Clic = ajouter/retirer un élément à la sélection, annoter, envoyer.");
}
