// Live Réglages screen — extracted from sift-live.ts (clean-architecture audit F1,
// 2026-07-09): this was one of several full-screen renderers still inlined in the
// god-module after ecartes-view.ts/journal.ts were split out.
// Self-contained: unlike Bibliothèque/Rekordbox, no state here is mutated from
// installLiveWiring's delegated click handler, so no cross-module state wiring is needed.
//
// Lu contre Réglages Système le 2026-09-09 (déclinaison #24, septième et dernier écran — spec
// `docs/ui-specs/reglages.md`, décision d'Antoine du 2026-09-02, § Décision 2026-09-09). La
// référence : une sidebar de catégories, un panneau à droite, application immédiate (guide macOS
// « Modifier les Réglages Système » : « une barre latérale à gauche contenant les catégories, un
// panneau principal à droite »). Ce que l'écran en fait :
//   · colonne B′ des catégories au PLAN DE LA FILE, bord à bord — la même règle CSS que la colonne
//     des sections de Rekordbox et des disques de Clé USB (co-sélecteur `.sift-settings-side`) ;
//   · panneau sans carte, borné à `--measure-form` (560 px, la mesure de formulaire), aligné en
//     tête de zone : le titre de la catégorie, sa phrase, puis des RANGÉES sur une grille commune —
//     libellé à gauche (150 px, la colonne de libellés de Rekordbox), contrôle à droite, alignés
//     d'une rangée à l'autre ;
//   · aucun bouton Enregistrer : le modèle de nommage s'enregistre à la frappe (débounce) et au
//     blur, comme le jeton Discogs le faisait déjà ; « Revenir au modèle par défaut » reste, en
//     action discrète ;
//   · ↑ ↓ déplacent la catégorie, Entrée/Espace la choisit ; un champ garde ses touches.
// Ce que cette lecture RETIRE : les deux cartes `.sift-ui-card-soft` (colonne et panneau — le
// panneau était la dernière carte de contenu de l'app), `.sift-settings-stack`, le libellé posé
// AU-DESSUS du champ (Discogs, Nommage), le bouton Enregistrer.
import { getSetting, setSetting, openUrl, previewFilename, verifyDiscogsToken } from "./ipc";
import { identifyErrorText } from "./identify-shared";
import { DEFAULT_FILENAME_TEMPLATE } from "../shared/contracts";
import type { Canonical } from "../shared/contracts";
import { requireEl, esc } from "./dom";
import { isStaleViewRender, viewEpoch } from "./view-epoch";
import { slideSegThumb } from "./seg-thumb";
import { setTheme } from "./theme";
import type { ThemeChoice } from "./theme";
import { toast } from "./filing-toast";
import { humanizeError } from "./errors";
import { open as openFolderDialog } from "@tauri-apps/plugin-dialog";
import { refreshRootWarning } from "./rail-root-warning";

/** Libellés des catégories, indexés par la clé `dataset.section` que chaque bloc porte déjà.
 *  Une clé sans libellé retombe sur la clé elle-même : une section neuve apparaît donc dans la
 *  colonne, mal nommée mais VISIBLE — un oubli qui se voit vaut mieux qu'une section introuvable.
 *
 *  Noms de la spec (§ Zone B′) : « Général » porte la racine de bibliothèque ; « Conversion »
 *  n'existe PAS — aucun réglage de conversion n'est stocké aujourd'hui, et une catégorie vide
 *  serait un mensonge. Elle arrivera avec son premier réglage. */
const SECTION_LABELS: Record<string, string> = {
  bibliotheque: "Général",
  nommage: "Nommage",
  discogs: "Identification",
  apparence: "Apparence",
};

/** Catégorie affichée. Au niveau module : l'écran se re-rend à chaque réglage appliqué (pas de
 *  bouton Enregistrer, application immédiate), et un état local retomberait sur la première
 *  catégorie à chaque frappe dans le champ de jeton. La première de la spec : Général. */
let activeSection = "bibliotheque";

/** Montre une seule section et marque son entrée. Les autres sont retirées du flux par `hidden`,
 *  pas seulement masquées : un champ dans une section cachée resterait tabulable. */
function selectSettingsCategory(key: string): void {
  activeSection = key;
  document.querySelectorAll<HTMLElement>("#sift-reglages-list > [data-section]").forEach((el) => {
    el.hidden = el.dataset.section !== key;
  });
  document.querySelectorAll<HTMLElement>('[data-reglages="cat"]').forEach((el) => {
    const on = el.dataset.cat === key;
    el.classList.toggle("on", on);
    el.setAttribute("aria-pressed", String(on));
  });
  // Apparence redevient visible : rejouer le placement du pouce du segmenté Thème. Il se mesure sur
  // `offsetWidth`/`offsetLeft`, tous deux 0 tant que la carte est `hidden` (display:none) — donc le
  // seul placement au render tombait à 0px quand une AUTRE catégorie était active à l'ouverture, et
  // rien ne le rejouait au changement de catégorie (seul un clic sur un bouton de thème le
  // réparait). Ici la carte vient de rentrer dans le flux, la mesure est enfin non nulle.
  if (key === "apparence") positionThemeThumb();
}

/** Place le pouce `.sift-seg-thumb` du segmenté Thème sur l'option active. Au niveau module (et non
 *  closure de `renderReglagesLive`) pour que `selectSettingsCategory` puisse le rejouer : il ne doit
 *  s'appeler QUE lorsque la carte Apparence est dans le flux — sur `display:none`, offsetWidth/Left
 *  valent 0 et le pouce se voit écrire un filet de 0px. */
function positionThemeThumb(): void {
  // Fusion du 2026-08-20 : le REPLAY au changement de catégorie vient d'une session parallèle
  // (eed4b26), le calcul partagé de la passe simplify (`seg-thumb.ts`, 6 copies fondues en une).
  const card = document.getElementById("sift-reglages-apparence");
  if (!card) return;
  slideSegThumb(card, "[data-theme-choice].on");
}

/** Appelée par le dispatch délégué de `sift-live.ts` au clic sur une catégorie. */
export function onSettingsCategoryPick(key: string): void {
  selectSettingsCategory(key);
}

/** Une rangée de la grille commune : libellé (et sa phrase, optionnelle) à gauche, contrôle à
 *  droite. `control` est du markup déjà échappé par l'appelant. */
function rowHtml(label: string, control: string, opts?: { note?: string; forId?: string }): string {
  const lab = opts?.forId
    ? `<label for="${esc(opts.forId)}" class="sift-settings-label">${label}</label>`
    : `<span class="sift-settings-label">${label}</span>`;
  return (
    `<div class="sift-settings-row"><div class="sift-settings-row-lab">${lab}` +
    (opts?.note ? `<span class="sift-settings-note">${opts.note}</span>` : "") +
    `</div><div class="sift-settings-row-ctl">${control}</div></div>`
  );
}

export async function renderReglagesLive() {
  const content = requireEl("#content", "renderReglagesLive");
  // `viewToken` et non `token` : ce module a déjà un `token`, celui de Discogs.
  const viewToken = viewEpoch();

  // Remove any previous live-settings wrapper so we don't duplicate on re-render.
  // All sections live inside this single wrapper (not as separate #content siblings)
  // so a future section can't be forgotten here the way libBlock/themeBlock once were.
  document.getElementById("sift-reglages-live")?.remove();
  const wrap = document.createElement("div");
  wrap.id = "sift-reglages-live";
  wrap.className = "sift-settings-panel";

  let token: string | null = null;
  try {
    token = await getSetting("discogs_token");
  } catch (e) {
    console.error("getSetting(discogs_token) failed", e);
  }
  let theme: ThemeChoice = "auto";
  try {
    const v = await getSetting("ui_theme");
    if (v === "light" || v === "dark") theme = v;
  } catch (e) {
    console.error("getSetting(ui_theme) failed", e);
  }
  let root: string | null = null;
  try {
    root = await getSetting("library_root");
  } catch (e) {
    console.error("getSetting(library_root) failed", e);
  }
  // Vide/absent = jamais personnalisé → on montre le défaut. `DEFAULT_FILENAME_TEMPLATE` vient de
  // `shared/contracts.ts`, miroir de `settings::DEFAULT_TEMPLATE` tenu par un test de contrat —
  // pas un littéral recopié ici.
  let tmpl = DEFAULT_FILENAME_TEMPLATE;
  try {
    const saved = await getSetting("filename_template");
    if (saved && saved.trim()) tmpl = saved;
  } catch (e) {
    console.error("getSetting(filename_template) failed", e);
  }

  // Dernier point d'attente avant que quoi que ce soit soit construit puis attaché à `#content`
  // (issue #42) : quatre `getSetting` séquentiels viennent de passer, et sous scan chacun attend le
  // `Mutex<Connection>`. Sans ce garde, la pile de sections de Réglages s'ajoutait au `#content` de
  // l'écran qu'on venait d'ouvrir. Le bloc synchrone en tête de fonction (retrait de
  // `#sift-reglages-live`) n'a PAS besoin du garde : aucun `await` ne le précède, il s'exécute donc
  // toujours dans le tour où l'écran est encore le sien.
  if (isStaleViewRender(viewToken)) return;

  // Divergence assumée : le jeton reste un input à sauvegarde auto (fonctionnel) au lieu du
  // "•••• 4471 + Modifier" de la maquette, dont le bouton est un onNotImpl de démo.
  const block = document.createElement("div");
  block.id = "sift-reglages-discogs";
  block.dataset.section = "discogs";
  block.className = "sift-settings-section";
  block.innerHTML =
    '<div class="sift-settings-title">Identification</div>' +
    // Impasse A9 (issue #15) : la phrase précédente — « Sans jeton, les recherches sont limitées
    // et plus lentes » — décrivait une désactivation TOTALE comme une dégradation. La réalité est
    // dans le code : `ipc_identify.rs` rend `NO_TOKEN` AVANT tout appel réseau, et `settings.rs`
    // le dit en toutes lettres, « Empty/unset = identification disabled ». Aucune recherche n'est
    // ni limitée ni ralentie : il n'y en a aucune.
    '<div class="sift-settings-desc">Le jeton permet à Sift d\'interroger l\'API Discogs pour identifier tes morceaux (label, année, genre). Sans jeton, Sift n\'interroge pas Discogs du tout : le bouton Identifier renvoie ici. Le jeton est gratuit et se génère depuis un compte Discogs.</div>' +
    rowHtml(
      "Jeton d'accès",
      // Masked like any credential (audit UI/UX 2026-07-03, fix 8) — a screenshot/share of Réglages
      // must not leak the token in clear text. Eye toggle to check it without retyping.
      '<div class="sift-settings-field">' +
        // class="sift-editor-input" instead of an inline-duplicated border/background (2026-07-10,
        // fix for a specificity bug this duplication caused: an inline `style="border:..."` always
        // beats a stylesheet rule, even :focus-visible, so this field's border silently didn't
        // shift color on focus while every other input using the shared class did).
        `<input id="sift-discogs-token" type="password" placeholder="Jeton Discogs…" value="${esc(token ?? "")}" class="sift-editor-input sift-settings-input sift-settings-input-secret">` +
        '<button type="button" id="sift-discogs-token-toggle" class="sift-settings-eye" title="Afficher le jeton" aria-label="Afficher le jeton"><i class="ti ti-eye" aria-hidden="true"></i></button>' +
        "</div>" +
        // « Vérifier » : impasse A11 de l'issue #15. Enregistrer un jeton ne dit que l'écriture ; sa
        // validité ne se découvrait qu'au premier Identifier, plus tard et dans un autre écran.
        // Libellé descriptif, donc TEXTE SEUL (règle CLAUDE.md : l'icône est réservée à ce qui n'a
        // pas d'équivalent textuel). Le bouton ne redéfinit aucun `background`, donc il garde le
        // `:hover` générique sans avoir à le réaffirmer.
        '<div class="sift-settings-subactions">' +
        '<button type="button" id="sift-discogs-verify">Vérifier</button>' +
        '<div id="sift-discogs-status" class="sift-settings-status"></div>' +
        "</div>",
      {
        forId: "sift-discogs-token",
        note: '<a id="sift-discogs-link" class="sift-settings-link">obtenir un jeton</a>',
      },
    );

  const libBlock = document.createElement("div");
  libBlock.id = "sift-reglages-bibliotheque";
  libBlock.dataset.section = "bibliotheque";
  libBlock.className = "sift-settings-section";
  libBlock.innerHTML =
    '<div class="sift-settings-title">Général</div>' +
    '<div class="sift-settings-desc">Le dossier racine est l\'endroit réel sur ton disque où Sift convertit les morceaux filés. L\'arborescence de destination (House/Deep, Techno…) vit à l\'intérieur. Les dossiers surveillés se gèrent depuis le rail, section Sources.</div>' +
    rowHtml(
      "Dossier racine",
      '<div class="sift-settings-field">' +
        `<span class="sift-settings-path${root ? "" : " sift-settings-path-empty"}">${esc(root || "Aucun dossier sélectionné")}</span>` +
        '<button id="sift-lib-root-change" type="button" class="sift-settings-btn">Changer…</button>' +
        "</div>" +
        (root
          ? '<div class="sift-settings-subactions"><button id="sift-lib-root-forget" type="button" class="sift-settings-btn sift-settings-btn-quiet">Oublier le dossier racine</button></div>'
          : "") +
        '<div id="sift-lib-root-status" class="sift-settings-status"></div>',
    );
  const libStatus = libBlock.querySelector<HTMLElement>("#sift-lib-root-status");
  libBlock.querySelector("#sift-lib-root-change")?.addEventListener("click", () => {
    void (async () => {
      const dir = await openFolderDialog({ directory: true, multiple: false });
      if (typeof dir !== "string") return;
      try {
        await setSetting("library_root", dir);
        // Le rappel du rail (#54) est peint depuis le RÉGLAGE, pas depuis un état de vue : le
        // relire ici est ce qui le fait tomber tout de suite, sans attendre un redémarrage.
        void refreshRootWarning();
        void renderReglagesLive();
      } catch (e) {
        if (libStatus) libStatus.textContent = "Erreur d'enregistrement.";
        console.error("setSetting(library_root) failed", e);
      }
    })();
  });
  libBlock.querySelector("#sift-lib-root-forget")?.addEventListener("click", () => {
    void (async () => {
      try {
        await setSetting("library_root", "");
        // Symétrique : oublier la racine doit REMETTRE le rappel, sur le même chemin.
        void refreshRootWarning();
        void renderReglagesLive();
      } catch (e) {
        if (libStatus) libStatus.textContent = "Erreur d'enregistrement.";
        console.error("setSetting(library_root) failed", e);
      }
    })();
  });

  // Deux pistes d'exemple : l'une AVEC version, l'autre sans. C'est le seul moyen de voir ce que
  // `{version}` fait réellement — y compris qu'il ne laisse pas de parenthèses vides quand la
  // piste n'en a pas.
  const TPL_SAMPLES: ReadonlyArray<{ c: Canonical; ext: string }> = [
    {
      c: { artist: "Chez Damier", title: "Can You Feel It", version: "Fluent Remix", label: null, confidence: "green" },
      ext: "aiff",
    },
    { c: { artist: "Mr Fingers", title: "Mystery of Love", version: null, label: null, confidence: "green" }, ext: "mp3" },
  ];

  const tplBlock = document.createElement("div");
  tplBlock.id = "sift-reglages-nommage";
  tplBlock.dataset.section = "nommage";
  tplBlock.className = "sift-settings-section";
  tplBlock.innerHTML =
    '<div class="sift-settings-title">Nommage</div>' +
    '<div class="sift-settings-desc">Le nom que Sift donne aux fichiers qu\'il range. Trois champs disponibles, à insérer d\'un clic. <code>{version}</code> se rend en «&nbsp;(Remix)&nbsp;» quand la piste en a une, et disparaît sinon — pas de parenthèses vides. Le modèle s\'enregistre à la frappe.</div>' +
    rowHtml(
      "Modèle",
      `<input id="sift-tpl-input" class="sift-editor-input sift-settings-input sift-tpl-input" spellcheck="false" aria-label="Modèle de nommage" value="${esc(tmpl)}">` +
        '<div class="sift-tpl-chips">' +
        ["{artist}", "{title}", "{version}"]
          .map((p) => `<button type="button" class="sift-tpl-chip" data-tpl-ph="${esc(p)}">${esc(p)}</button>`)
          .join("") +
        "</div>" +
        '<div id="sift-tpl-warn" class="sift-tpl-warn" hidden></div>',
      { forId: "sift-tpl-input" },
    ) +
    rowHtml(
      "Aperçu",
      '<div id="sift-tpl-preview" class="sift-tpl-preview"></div>' +
        '<div class="sift-settings-subactions">' +
        '<button type="button" id="sift-tpl-reset" class="sift-settings-btn sift-settings-btn-quiet">Revenir au modèle par défaut</button>' +
        '<div id="sift-tpl-status" class="sift-settings-status"></div>' +
        "</div>",
    );

  const tplInput = tplBlock.querySelector<HTMLInputElement>("#sift-tpl-input");
  const tplPreview = tplBlock.querySelector<HTMLElement>("#sift-tpl-preview");
  const tplWarn = tplBlock.querySelector<HTMLElement>("#sift-tpl-warn");
  const tplStatus = tplBlock.querySelector<HTMLElement>("#sift-tpl-status");

  // Les deux lignes d'aperçu sont créées UNE fois ; le handler de frappe ne mute que leur
  // `textContent`. `input` est un événement en rafale (une frappe = un tir) : reconstruire le DOM
  // ici saturerait le thread UI, cf. CLAUDE.md § Front — événements répétés.
  const tplLines = TPL_SAMPLES.map(() => {
    const el = document.createElement("div");
    el.className = "sift-tpl-preview-line";
    tplPreview?.appendChild(el);
    return el;
  });

  /** Avertissement, jamais un blocage : retirer un champ est légitime si on sait ce qu'on fait —
   *  l'aperçu montre déjà la conséquence, et `ensure_unique` gère la collision côté rangement. */
  function tplWarning(t: string): string {
    if (!t.trim()) return "Un modèle vide n'est pas utilisable.";
    if (!t.includes("{title}"))
      return "Sans {title}, deux morceaux du même artiste produisent le même nom — Sift ajoutera un suffixe numérique pour éviter l'écrasement.";
    if (!t.includes("{artist}"))
      return "Sans {artist}, les reprises et remixes d'un même titre se retrouvent côte à côte sans distinction.";
    return "";
  }

  // Un aperçu = 2 appels IPC. Débounce pour ne pas en tirer un par frappe, et garde de séquence
  // pour qu'une réponse lente n'écrase pas le résultat d'une frappe plus récente.
  let tplDebounce: ReturnType<typeof setTimeout> | undefined;
  let tplSeq = 0;
  function refreshTplPreview(): void {
    const t = tplInput?.value ?? "";
    const w = tplWarning(t);
    if (tplWarn) {
      tplWarn.textContent = w;
      tplWarn.hidden = !w;
    }
    clearTimeout(tplDebounce);
    tplDebounce = setTimeout(() => {
      const mine = ++tplSeq;
      void Promise.all(TPL_SAMPLES.map((s) => previewFilename(s.c, s.ext, t)))
        .then((names) => {
          if (mine !== tplSeq) return; // une frappe plus récente a déjà répondu
          names.forEach((n, i) => {
            const line = tplLines[i];
            if (line) line.textContent = `→ ${n}`;
          });
        })
        .catch((e: unknown) => {
          if (mine !== tplSeq) return;
          console.error("[preview_filename] aperçu du modèle", e);
          tplLines.forEach((l) => {
            l.textContent = "→ aperçu indisponible";
          });
        });
    }, 120);
  }

  // Application immédiate (spec § Zone C : « aucun bouton Enregistrer ») — même paire
  // débounce + blur que le jeton Discogs. Un modèle vide n'est pas écrit : l'avertissement le dit
  // déjà, et la dernière valeur valide reste en base.
  let tplSaveTimer: ReturnType<typeof setTimeout> | undefined;
  let tplLastSaved = tmpl;
  async function saveTemplate(): Promise<void> {
    clearTimeout(tplSaveTimer);
    const t = tplInput?.value ?? "";
    if (!t.trim() || t === tplLastSaved) return;
    try {
      await setSetting("filename_template", t);
      tplLastSaved = t;
      if (tplStatus) {
        tplStatus.textContent = "Modèle enregistré.";
        setTimeout(() => {
          if (tplStatus && tplStatus.textContent === "Modèle enregistré.") tplStatus.textContent = "";
        }, 2000);
      }
    } catch (e) {
      console.error("[setSetting(filename_template)] enregistrement", e);
      if (tplStatus) tplStatus.textContent = "Échec de l'enregistrement — réessaie.";
    }
  }

  tplInput?.addEventListener("input", () => {
    refreshTplPreview();
    clearTimeout(tplSaveTimer);
    tplSaveTimer = setTimeout(() => void saveTemplate(), 600);
  });
  tplInput?.addEventListener("blur", () => void saveTemplate());
  tplBlock.querySelectorAll<HTMLElement>("[data-tpl-ph]").forEach((chip) => {
    chip.addEventListener("click", () => {
      if (!tplInput) return;
      const ph = chip.dataset.tplPh ?? "";
      const s = tplInput.selectionStart ?? tplInput.value.length;
      const e = tplInput.selectionEnd ?? s;
      tplInput.value = tplInput.value.slice(0, s) + ph + tplInput.value.slice(e);
      tplInput.focus();
      const caret = s + ph.length;
      tplInput.setSelectionRange(caret, caret);
      refreshTplPreview();
      void saveTemplate();
    });
  });
  tplBlock.querySelector("#sift-tpl-reset")?.addEventListener("click", () => {
    if (!tplInput) return;
    tplInput.value = DEFAULT_FILENAME_TEMPLATE;
    refreshTplPreview();
    void saveTemplate();
  });
  refreshTplPreview();

  const themeBlock = document.createElement("div");
  themeBlock.id = "sift-reglages-apparence";
  themeBlock.dataset.section = "apparence";
  themeBlock.className = "sift-settings-section";
  // Audit-ref G1 (Réglages, 2026-07-09) : <span> → <button>, incohérent avec le reste de l'app.
  const themeBtn = (v: ThemeChoice, label: string) =>
    `<button class="sift-seg-opt${theme === v ? " on" : ""}" data-theme-choice="${v}">${label}</button>`;
  // Audit-ref (Réglages, 2026-07-09, retour Antoine "on n'a pas l'animation pour toutes les
  // pastilles") : thumb glissant ajouté ici — son DOM persiste déjà entre les clics (classList
  // toggle en place, pas de re-render), donc éligible sans restructuration (contrairement à
  // Dossiers/Genres et Session/Historique, qui reconstruisent tout via innerHTML à chaque clic —
  // voir css-transition-requires-persisting-dom en mémoire). Même pattern que positionFmtThumb().
  themeBlock.innerHTML =
    '<div class="sift-settings-title">Apparence</div>' +
    '<div class="sift-settings-desc">Auto suit le réglage clair/sombre de ton système. Clair et Sombre forcent un mode fixe, quel que soit le système.</div>' +
    rowHtml(
      "Thème",
      '<div class="sift-seg sift-seg-thumbed">' +
        '<div class="sift-seg-thumb"></div>' +
        themeBtn("auto", "Auto") +
        themeBtn("light", "Clair") +
        themeBtn("dark", "Sombre") +
        "</div>",
    );
  themeBlock.querySelectorAll<HTMLElement>("[data-theme-choice]").forEach((el) =>
    el.addEventListener("click", () => {
      const choice = el.dataset.themeChoice as ThemeChoice;
      // Le `.on` suit l'APPLICATION, qui est immédiate et ne peut pas échouer — le thème demandé
      // est bien celui à l'écran. Ce que le bouton allumé ne dit pas, c'est si le choix a été
      // ENREGISTRÉ ; impasse A21 (issue #15), où l'échec d'écriture ne se voyait qu'au lancement
      // suivant, quand le thème revenait tout seul.
      void setTheme(choice).then((r) => {
        if (r.persisted) return;
        toast(
          humanizeError(
            r.error,
            "Thème appliqué, mais pas enregistré : il reviendra à sa valeur précédente au prochain lancement.",
            "setTheme",
          ),
        );
      });
      themeBlock.querySelectorAll("[data-theme-choice]").forEach((c) => c.classList.remove("on"));
      el.classList.add("on");
      positionThemeThumb();
    }),
  );

  // NB : la carte « Formater une clé USB » a quitté cet écran le 2026-07-31 — elle est le contenu
  // de l'onglet Clé USB (`usb-view.ts`), qui a désormais son propre écran. Ne pas la réintroduire
  // ici : tout ce qui touche la clé USB vit dans cet onglet, une seule source.

  // Single wrapper: only #sift-reglages-live is removed/recreated per render (see the
  // 2026-07-04 fix), so every settings section — present or future — must build inside `wrap`
  // rather than as a direct sibling of `content`, or it duplicates on re-render.
  //
  // 2026-07-08 : les 4 sections étaient chacune leur propre .sift-ui-card-soft, puis une seule
  // carte partagée ; 2026-09-09 : plus de carte du tout — la zone C ne peint rien (Rangés,
  // Rekordbox, Clé USB), et le panneau est le dernier écran qui en portait une. Le rythme vertical
  // vient de la grille des rangées. Toute nouvelle section s'ajoute à l'intérieur de `list`.
  const list = document.createElement("div");
  list.id = "sift-reglages-list";
  list.className = "sift-settings-list";
  // Ordre de la spec (§ Zone B′) : Général · Nommage · Identification · Apparence.
  const sections = [libBlock, tplBlock, block, themeBlock];
  for (const el of sections) list.appendChild(el);
  wrap.appendChild(list);

  // DEUX COLONNES depuis l'étape 9 (DESIGN.md § 17, question ouverte O-3) ; la colonne au plan de
  // la file depuis le 2026-09-09.
  //
  // L'écran était une colonne unique plafonnée à 560px, qui laissait 44 % de la fenêtre vide sur
  // 1200 (rail 152 + padding 2×24 retirés : 1000 utiles, 560 employés). La correction n'était PAS
  // d'élargir la colonne : Réglages Système emploie justement un panneau étroit — mais à côté
  // d'une sidebar de catégories. Ce qui manquait n'était pas de la largeur, c'était la seconde
  // colonne. Le panneau garde donc sa mesure de formulaire ; il est accompagné.
  //
  // Les catégories sont DÉRIVÉES des sections déjà rendues (`dataset.section`), jamais d'une table
  // parallèle : une liste écrite ici divergerait à la première section ajoutée, exactement comme
  // l'aurait fait une table vue → titre dans le routeur.
  const layout = document.createElement("div");
  layout.className = "sift-settings-layout";
  const side = document.createElement("nav");
  side.className = "sift-settings-side";
  side.setAttribute("aria-label", "Catégories de réglages");
  side.innerHTML = `<div class="col-h">Réglages</div>`;
  for (const el of sections) {
    const key = el.dataset.section ?? "";
    const label = SECTION_LABELS[key] ?? key;
    side.insertAdjacentHTML(
      "beforeend",
      `<div class="fld" data-reglages="cat" data-cat="${esc(key)}" tabindex="0" role="button" aria-pressed="false">${esc(label)}</div>`,
    );
  }
  // Clavier (spec § Interactions) : ↑ ↓ déplacent la catégorie, Entrée/Espace la choisit. Un
  // champ du panneau n'est jamais concerné : le listener vit sur la colonne seule.
  side.addEventListener("keydown", (e) => {
    const cur = (e.target as HTMLElement).closest<HTMLElement>('[data-reglages="cat"]');
    if (!cur) return;
    const all = Array.from(side.querySelectorAll<HTMLElement>('[data-reglages="cat"]'));
    const i = all.indexOf(cur);
    let next: HTMLElement | undefined;
    if (e.key === "ArrowDown") next = all[i + 1];
    else if (e.key === "ArrowUp") next = all[i - 1];
    else if (e.key === "Enter" || e.key === " ") next = cur;
    else return;
    e.preventDefault();
    if (!next) return;
    next.focus();
    selectSettingsCategory(next.dataset.cat ?? "");
  });
  const main = document.createElement("div");
  main.className = "sift-settings-main";
  main.appendChild(wrap);
  layout.appendChild(side);
  layout.appendChild(main);
  content.appendChild(layout);
  // Montre la catégorie active ET, si c'est « apparence », place le pouce du segmenté Thème
  // maintenant qu'il est dans le flux (selectSettingsCategory s'en charge).
  selectSettingsCategory(activeSection);

  const inp = block.querySelector<HTMLInputElement>("#sift-discogs-token");
  const status = block.querySelector<HTMLElement>("#sift-discogs-status");
  const link = block.querySelector<HTMLElement>("#sift-discogs-link");
  const toggle = block.querySelector<HTMLButtonElement>("#sift-discogs-token-toggle");
  const verify = block.querySelector<HTMLButtonElement>("#sift-discogs-verify");

  verify?.addEventListener("click", () => {
    void (async () => {
      if (!status) return;
      // Le jeton est écrit AVANT d'être vérifié : sans ça, un clic direct après la frappe
      // vérifierait la valeur précédente, puisque `verify_discogs_token` lit les réglages et non
      // le champ. Le débounce de 600 ms rend ce cas parfaitement atteignable.
      await saveToken();
      verify.disabled = true;
      status.textContent = "Vérification…";
      status.style.color = "var(--color-text-tertiary)";
      try {
        await verifyDiscogsToken();
        status.textContent = "Jeton accepté par Discogs.";
        status.style.color = "var(--color-text-success)";
      } catch (e) {
        const { texte, grave } = identifyErrorText(e);
        status.textContent = texte;
        status.style.color = grave
          ? "var(--color-text-danger)"
          : "var(--color-text-tertiary)";
      } finally {
        verify.disabled = false;
      }
    })();
  });

  toggle?.addEventListener("click", () => {
    if (!inp) return;
    const shown = inp.type === "text";
    inp.type = shown ? "password" : "text";
    toggle.title = shown ? "Afficher le jeton" : "Masquer le jeton";
    toggle.setAttribute("aria-label", toggle.title);
    toggle.innerHTML = `<i class="ti ${shown ? "ti-eye" : "ti-eye-off"}" aria-hidden="true"></i>`;
  });

  link?.addEventListener("click", () =>
    void openUrl("https://www.discogs.com/settings/developers").catch((e) =>
      console.error("openUrl failed", e),
    ),
  );

  let saveTimer: ReturnType<typeof setTimeout> | undefined;

  /** Écrit le jeton et le dit. Annule le débounce en cours pour que l'appel immédiat du `blur` ne
   *  se fasse pas doubler par le timer qui allait échoir. */
  async function saveToken(): Promise<void> {
    clearTimeout(saveTimer);
    if (!inp) return;
    const val = inp.value.trim();
    try {
      await setSetting("discogs_token", val);
      if (status) {
        // Ce libellé ne dit QUE ce qui s'est passé : l'écriture. Il ne dit pas que le jeton est
        // valide — rien ici ne l'a testé. Ce qu'il vaut se découvre au premier Identifier, qui
        // sait maintenant distinguer un jeton refusé d'une panne réseau (impasse A10, issue #15).
        status.textContent = val ? "Jeton enregistré." : "Jeton effacé.";
        setTimeout(() => {
          if (status) status.textContent = "";
        }, 2000);
      }
    } catch (e) {
      if (status) status.textContent = "Erreur d'enregistrement.";
      console.error("setSetting(discogs_token) failed", e);
    }
  }

  inp?.addEventListener("input", () => {
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => void saveToken(), 600);
  });
  // Moitié débounce de l'impasse A11 (issue #15) : le timer de 600 ms n'était vidé ni à la
  // navigation ni à la fermeture, donc coller un jeton puis quitter l'écran sous 600 ms le perdait
  // SANS TRACE — ni message, ni log, et le champ réaffichait l'ancienne valeur au retour. Un clic
  // sur le rail de navigation retire le focus du champ avant de démonter l'écran : c'est ce `blur`
  // qui rattrape la saisie. (Le retrait du DOM seul ne déclenche pas `blur` dans Chromium — donc
  // c'est bien l'ordre des événements du clic qui porte la garantie, pas le démontage.)
  inp?.addEventListener("blur", () => void saveToken());
}
