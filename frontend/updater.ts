import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { esc } from "./dom";

const BANNER_ID = "sift-update-banner";

/** Longueur au-delà de laquelle le résumé est coupé. La bannière est une ligne : au-delà elle
 *  pousse les deux boutons hors de la fenêtre sur un écran étroit — le défaut que `fix(updater):
 *  keep the update banner from clipping on narrow windows` a déjà corrigé une fois. */
const NOTES_MAX_CHARS = 90;

/** Première ligne utile des notes de version, ramenée à une phrase courte.
 *
 *  Les notes sont du Markdown (section de `CHANGELOG.md`) : on ne le rend PAS — la bannière
 *  n'est pas un lecteur de changelog, et rendre du Markdown venu du réseau ouvrirait une surface
 *  d'injection pour économiser une ligne de texte. On prend le premier titre de sous-section, ou
 *  à défaut la première ligne de prose, débarrassée de ses marqueurs.
 *
 *  Retourne `""` quand il n'y a rien de présentable — l'appelant n'affiche alors rien du tout,
 *  plutôt qu'un espace vide ou un tiret orphelin. */
function summariseNotes(body: string | undefined): string {
  if (!body) return "";
  for (const raw of body.split(/\r?\n/)) {
    const line = raw
      .trim()
      .replace(/^#{1,6}\s*/, "") // titre Markdown
      .replace(/^[-*]\s+/, "") // puce
      .replace(/\*\*/g, "") // gras
      .replace(/`/g, "")
      .trim();
    // `---` sépare le pied de page d'installation : tout ce qui suit n'est pas un changement.
    if (line === "---") return "";
    if (!line) continue;
    return line.length > NOTES_MAX_CHARS ? `${line.slice(0, NOTES_MAX_CHARS - 1).trimEnd()}…` : line;
  }
  return "";
}

function renderBanner(update: Update): void {
  document.getElementById(BANNER_ID)?.remove();
  const el = document.createElement("div");
  el.id = BANNER_ID;
  el.className = "sift-update-banner";
  el.setAttribute("role", "status");
  el.setAttribute("aria-live", "polite");
  // `update.body` porte les notes de version : `releaseBody` de release.yml, extrait de
  // CHANGELOG.md, recopié par tauri-action dans le champ `notes` de `latest.json`. Texte
  // d'origine EXTERNE (téléchargé depuis GitHub) rendu par innerHTML — `esc()` obligatoire.
  //
  // Tronqué : la bannière est une barre, pas un écran de notes. Le changelog complet vit sur la
  // page de la release, et le lire n'est pas ce qu'on demande à quelqu'un avant d'installer.
  const notes = summariseNotes(update.body);
  el.innerHTML =
    `<span>Mise à jour ${esc(update.version)} disponible.` +
    (notes ? ` <span class="sift-update-banner-notes">${esc(notes)}</span>` : "") +
    "</span>" +
    '<button data-upd="install" class="sift-update-banner-install">Installer et redémarrer</button>' +
    '<button data-upd="later" class="sift-update-banner-later">Plus tard</button>';
  document.body.appendChild(el);

  el.querySelector('[data-upd="later"]')?.addEventListener("click", () => {
    el.remove();
  });

  el.querySelector('[data-upd="install"]')?.addEventListener("click", () => {
    void installAndRelaunch(update, el);
  });
}

async function installAndRelaunch(update: Update, banner: HTMLElement): Promise<void> {
  const span = banner.querySelector("span");
  const installBtn = banner.querySelector('[data-upd="install"]') as HTMLButtonElement | null;

  if (installBtn) {
    installBtn.disabled = true;
    if (span) span.textContent = "Téléchargement…";
  }

  try {
    await update.downloadAndInstall();
    if (span) span.textContent = "Installation terminée, redémarrage...";
    await relaunch();
  } catch (e) {
    // .textContent, not .innerHTML — the browser escapes it on assignment, so esc() here
    // would double-encode entities (literal "&amp;" shown to the user instead of "&").
    if (span) span.textContent = `Échec de la mise à jour : ${String(e)}`;
    if (installBtn) installBtn.disabled = false;
    console.error("update install failed", e);
  }
}

/** Checks for an update once, at app launch. Called only from the `inTauri` block in
 *  main.ts — this module talks to the real updater plugin and has no meaning outside
 *  a running Tauri shell. No periodic re-check: release cadence is one-off, not
 *  scheduled (design.md, Contexte). */
export async function installUpdateBanner(): Promise<void> {
  try {
    const update = await check();
    if (update?.available) {
      renderBanner(update);
    }
  } catch (e) {
    // Silent: two expected causes, neither worth interrupting the user for. (1) No network /
    // GitHub unreachable — the real-world case on a signed release build. (2) The updater plugin
    // itself never registered — happens on every dev/unsigned-CI build, where plugins.updater has
    // no config to merge (see lib.rs's setup()); the IPC command this calls simply doesn't exist
    // there. Logged only, both cases.
    console.error("update check failed", e);
  }
}
