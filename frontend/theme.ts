// Theme (light/dark) — auto by default (follows OS via prefers-color-scheme, see styles.css),
// overridable per-user from Réglages. The choice persists via the same settings store as the
// Discogs token (getSetting/setSetting), keyed "ui_theme".
import { getSetting, setSetting } from "./ipc";

export type ThemeChoice = "auto" | "light" | "dark";
const THEME_SETTING = "ui_theme";

/** Apply a choice to the document: "auto" clears the override so the CSS media query decides;
 *  "light"/"dark" force it via [data-theme], regardless of the OS preference. */
function apply(choice: ThemeChoice): void {
  if (choice === "auto") delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = choice;
}

/** Read the persisted choice (default "auto") and apply it. Call once at boot. */
export async function initTheme(): Promise<ThemeChoice> {
  let choice: ThemeChoice = "auto";
  try {
    const v = await getSetting(THEME_SETTING);
    if (v === "light" || v === "dark") choice = v;
  } catch (e) {
    console.error("getSetting(ui_theme) failed", e);
  }
  apply(choice);
  return choice;
}

/** Apply + persist a new choice (Réglages toggle).
 *
 *  Rend le résultat de la PERSISTANCE, pas celui de l'application : les deux réussissent
 *  séparément. `apply()` ne peut pas échouer (elle écrit un attribut du document), donc le thème
 *  demandé est toujours à l'écran au retour ; `setSetting` traverse l'IPC et la base, et peut
 *  échouer seule.
 *
 *  Impasse A21 (issue #15) : ce `catch` était `console.error` seul, et le segmented control
 *  basculait son `.on` inconditionnellement. L'utilisateur voyait donc une préférence confirmée
 *  — thème appliqué, bouton allumé — qui était perdue au lancement suivant. L'appelant a besoin
 *  de la distinction pour le dire ; il ne peut pas la deviner. */
export async function setTheme(
  choice: ThemeChoice,
): Promise<{ persisted: true } | { persisted: false; error: unknown }> {
  apply(choice);
  try {
    await setSetting(THEME_SETTING, choice);
    return { persisted: true };
  } catch (e) {
    // Pas de `console.error` ici : c'est `humanizeError` chez l'appelant qui garantit la chaîne
    // brute en console, et journaliser aux deux endroits doublerait chaque échec.
    return { persisted: false, error: e };
  }
}
