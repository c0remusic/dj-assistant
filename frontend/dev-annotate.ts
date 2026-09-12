// Dev-only: capture de contexte pour le pointeur visuel d'annotation (Alt+Clic).
// Produit des VALEURS exactes (getComputedStyle) plutôt qu'une image — voir
// docs/superpowers/specs/2026-07-05-visual-pointer-annotation-design.md.
import { invoke } from "@tauri-apps/api/core";

interface ElementCapture {
  tag: string;
  id: string | null;
  classes: string[];
  text: string;
  styles: Record<string, string>;
  rect: { w: number; h: number };
}

interface ElementContext {
  element: ElementCapture;
  ancestors: { tag: string; id: string | null; classes: string[] }[];
  siblings: ElementCapture[];
  code: { file: string; line: number; excerpt: string }[];
}

export interface Annotation {
  note: string;
  view: string | null;
  // Un seul élément dans le cas courant, plusieurs si sélection multi (Alt+Clic répété) —
  // pour un problème qui ne s'explique qu'en comparant deux zones (ex. incohérence entre écrans).
  elements: ElementContext[];
}

// Propriétés pertinentes pour un problème visuel — pas les ~350 brutes.
const STYLE_PROPS = [
  "color", "background-color",
  "font-family", "font-size", "font-weight", "line-height",
  "margin-top", "margin-right", "margin-bottom", "margin-left",
  "padding-top", "padding-right", "padding-bottom", "padding-left",
  "border-top-width", "border-top-color", "border-radius",
  "gap", "display", "align-items", "justify-content",
  "opacity", "box-shadow",
] as const;

function captureElement(el: HTMLElement): ElementCapture {
  const cs = getComputedStyle(el);
  const styles: Record<string, string> = {};
  for (const p of STYLE_PROPS) styles[p] = cs.getPropertyValue(p);
  const r = el.getBoundingClientRect();
  return {
    tag: el.tagName.toLowerCase(),
    id: el.id || null,
    classes: [...el.classList],
    text: (el.textContent ?? "").trim().slice(0, 120),
    styles,
    rect: { w: Math.round(r.width), h: Math.round(r.height) },
  };
}

function activeView(): string | null {
  const on = document.querySelector<HTMLElement>("#nav .nv.on");
  return on?.dataset.view ?? null;
}

async function buildElementContext(el: HTMLElement): Promise<ElementContext> {
  const ancestors: ElementContext["ancestors"] = [];
  for (let a = el.parentElement; a && a !== document.body && ancestors.length < 8; a = a.parentElement) {
    ancestors.push({ tag: a.tagName.toLowerCase(), id: a.id || null, classes: [...a.classList] });
  }
  const siblings = [...(el.parentElement?.children ?? [])]
    .filter((s): s is HTMLElement => s !== el && s instanceof HTMLElement)
    .slice(0, 6)
    .map(captureElement);

  // Localisation code : même identifiants que l'inspecteur (id d'abord, puis classes).
  const identifiers = el.id ? [`#${el.id}`, ...el.classList] : [...el.classList];
  const code: ElementContext["code"] = [];
  for (const ident of identifiers.slice(0, 3)) {
    try {
      const matches = await invoke<{ file: string; line: number; excerpt: string }[]>(
        "locate_source", { identifier: ident },
      );
      code.push(...matches);
    } catch {
      // fail-fast affiché au moment de l'envoi si TOUT échoue ; un identifiant
      // sans résultat n'est pas une erreur (spec: localisation vide tolérée).
    }
    if (code.length >= 20) break;
  }

  return { element: captureElement(el), ancestors, siblings, code: code.slice(0, 20) };
}

export async function buildAnnotation(els: HTMLElement[], note: string): Promise<Annotation> {
  const elements: ElementContext[] = [];
  for (const el of els) elements.push(await buildElementContext(el));
  return { note, view: activeView(), elements };
}

export async function sendAnnotation(annotation: Annotation): Promise<void> {
  await invoke("save_annotation", { annotation });
}
