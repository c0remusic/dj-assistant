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

/** Apply + persist a new choice (Réglages toggle). */
export async function setTheme(choice: ThemeChoice): Promise<void> {
  apply(choice);
  try {
    await setSetting(THEME_SETTING, choice);
  } catch (e) {
    console.error("setSetting(ui_theme) failed", e);
  }
}
