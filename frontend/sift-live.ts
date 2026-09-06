// Live data wiring — ACTIVE ONLY inside the Tauri app. In a plain browser the hooks
// below are never installed, so app.js keeps its mockup (Vercel demo unaffected).
import {
  onFileDone,
  onFileProgress,
  onQueueChanged,
  onScanFailed,
  onAnalysisChanged,
  analysisProgress,
  trashTrack,
  restoreTrack,
  requeueTrack,
  purgeTrash,
  openUrl,
  exportRekordboxXml,
  linkRekordboxXml,
  rekordboxStatus,
  revealTrack,
} from "./ipc";
import { installUndoShortcut, installFilingKeys, registerAddSourceAction } from "./filing";
import { refreshBinsForBatch } from "./filing-bins";
import { confirmAction } from "./confirm-modal";
// Views/chrome extracted from this god-module (audit P-3) — kept stateless, wired here.
import { renderEcartes } from "./ecartes-view";
import { installDragDrop, injectLeanStyle, injectTitlebar, installScrollAutohide, installNavKeyboard, installRailToggle } from "./chrome";
import { initTheme } from "./theme";
import { installRailSources, renderRailSources, noteScanFailure, pickAndAddFolder } from "./rail-sources";
import {
  applyRowClick,
  renderSelectionSummary,
  openBiblioContextMenu,
  openColumnHeaderMenu,
  paintBibSelection,
  toggleFacetPopover,
  closeFacetPopover,
  keepFacetPopoverOpen,
  installFacetPopoverDismiss,
} from "./bibliotheque-view";
import { sortTracks } from "./library-views";
import { consumeSortSuppression } from "./library-columns";
import { refreshRootWarning } from "./rail-root-warning";
import { registerOpenSettingsAction } from "./filing-actions";
import { goTo } from "./router";
import { onSettingsCategoryPick } from "./reglages-view";
import { onRekordboxSectionPick } from "./rekordbox-view";
import { installWindowShortcuts } from "./shortcuts";
import { requireEl } from "./dom";
import { toast, copyToClipboard } from "./filing-toast";
import { humanizeError } from "./errors";
import type { LibrarySortState } from "./library-views";
import {
  bibState,
  bibDup,
  loadDuplicates,
  renderBiblioLive,
  openBiblioDetail,
  positionViewModeThumb,
} from "./bibliotheque-view";
import { renderRekordboxLive, handleRekordboxAction } from "./rekordbox-view";
import {
  enterDetailMode,
  enterBatchMode,
  registerBatchRenderer,
  renderQueue,
  handleQueueItemClick,
  installQueueNavKeys,
  reanalyzeTrack,
  refreshQueueFootStat,
} from "./queue-panel";
import {
  renderBatch,
  batchBin,
  batchInPlace,
  onBatchBinPick,
  handleBatchAction,
  handleBatchQueueAction,
  pushFileProgress,
  onFileStop,
  onFileBatchDone,
  registerRefreshHook,
  onBatchInPlaceChange,
} from "./batch-panel";
import { open as openFolderDialog } from "@tauri-apps/plugin-dialog";
import { dirname } from "@tauri-apps/api/path";
import { setTask, clearTask, setCancelHandler } from "./progress-zone";

// Global progress zone — feed the "analyze" row from the EXISTING analysis poll/events (no engine
// rewrite). `analysis_progress` returns (done, total) over PENDING tracks; a track stays pending
// after it's analysed (until filed), so done==total is the RESTING state, not "busy". So we show
// the row only while done<total (actively analysing), then flash a brief 100% "done" before hiding.
let analyzeWasRunning = false;
let analyzeClearTimer: ReturnType<typeof setTimeout> | undefined;
async function pushAnalyzeProgress() {
  try {
    const p = await analysisProgress();
    if (p.total > 0 && p.done < p.total) {
      clearTimeout(analyzeClearTimer);
      analyzeWasRunning = true;
      setTask("analyze", { done: p.done, total: p.total, state: "running" });
    } else if (analyzeWasRunning) {
      // Reached done==total (or the queue drained): flash 100% then auto-hide the row.
      analyzeWasRunning = false;
      setTask("analyze", { done: p.total, total: p.total, state: "done" });
      clearTimeout(analyzeClearTimer);
      analyzeClearTimer = setTimeout(() => clearTask("analyze"), 1200);
    } else {
      clearTask("analyze");
    }
    // La rangée de statut du pied de file rend cet état aussi (« Analyse — x/y » + barre,
    // queue-panel.ts) : la resynchroniser à la cadence de CE poll, pas à celle des re-rendus.
    refreshQueueFootStat();
  } catch (e) {
    console.error("analysisProgress failed", e);
  }
}

/** Guards a single in-flight Rekordbox export run. */
let exportRunning = false;

/** Rekordbox export (real merge+rewrite via `export_rekordbox_xml`, called from the Rekordbox
 * page's "Réexporter maintenant" button — see renderRekordboxLive). The "Clé USB" nav item no
 * longer routes here (finding F5, audit-heuristique-visuel.md) — it has its own screen since
 * 2026-07-31 (`usb-view.ts`). */
async function runNavExport(): Promise<void> {
  if (exportRunning) return; // one export run at a time
  exportRunning = true;
  setTask("export", { done: 0, total: 1, state: "running" });
  try {
    const status = await exportRekordboxXml();
    setTask("export", { done: 1, total: 1, state: "done" });
    setTimeout(() => clearTask("export"), 1200);
    toast(
      `${status.track_count} pistes dans ${status.playlist_count} playlists Rekordbox — réimporte le XML dans Rekordbox pour resynchroniser.`,
    );
  } catch (e) {
    console.error("export_rekordbox_xml failed", e);
    setTask("export", { done: 0, total: 1, state: "error" });
    const msg = e instanceof Error ? e.message : String(e);
    toast(
      msg.includes("aucun XML")
        ? // La commande de liaison (`rkblink`) vit sur l'écran Rekordbox, pas dans la
          // Bibliothèque : le message renvoyait vers un écran où elle n'est pas. Le libellé est
          // celui de l'item de navigation (`index.html`, `<span>Rekordbox</span>`) — pas
          // « Mettre à jour Rekordbox », qui n'est qu'un bouton de la carte de stats d'Accueil.
          "Aucun XML Rekordbox lié — relie un fichier depuis l'écran Rekordbox"
        : `Export Rekordbox échoué : ${msg}`,
    );
  } finally {
    exportRunning = false;
  }
}

/** Switch between detail and batch review. On entering batch the #fldz tree becomes the destination
 * explorer (batch pick mode); on leaving we restore the per-track filing pane.
 *
 * Les DEUX branches sont maintenant des portes de `queue-panel.ts` — `enterDetailMode()` et
 * `enterBatchMode()` (2026-08-26) —, et chacune finit par sa propre repeinture de la file. Cette
 * symétrie est délibérée : c'est son absence qui avait laissé la branche batch repeindre `#mid` et
 * `#fldz` en oubliant la colonne, bug invisible aux gates (voir `enterBatchMode`).
 *
 * Ce qui RESTE ici est ce que `queue-panel.ts` ne peut pas atteindre sans cycle d'import :
 * `refreshBinsForBatch` lit `batchBin`/`batchInPlace`, propriété de `batch-panel.ts` — le renderer,
 * lui, franchit la frontière par `registerBatchRenderer` (Phase 1, tranche 1c).
 *
 * Plus de `ensureReviewSeg()` ici depuis le 2026-08-25 : le segmenté Détail / Lot de la colonne
 * file a été retiré (`docs/ui-specs/revue.md` §§ Zone A / Zone B′), et c'est l'icône de sélection
 * de la barre unifiée qui porte l'état armé — repeinte en fin de fonction, sur les DEUX branches. */
function setReviewMode(m: "detail" | "batch") {
  if (m === "batch") {
    // La séquence d'armement vit dans `enterBatchMode` (queue-panel.ts) depuis le 2026-08-26, en
    // face d'`enterDetailMode` : c'est là qu'est la repeinture de la file, qui manquait ici et
    // rendait les cases de ligne invisibles jusqu'au prochain repaint fortuit. Ne rien remettre de
    // cette séquence ici — la symétrie des deux portes EST la garde.
    const fldz = requireEl("#fldz", "setReviewMode");
    enterBatchMode();
    void refreshBinsForBatch(fldz, batchBin, onBatchBinPick, batchInPlace);
  } else {
    enterDetailMode();
  }
  // Plus de repeinture du commutateur ici : les deux portes appellent `syncQueueSelectButton`
  // elles-mêmes, donc TOUS les chemins de changement de mode le repeignent — y compris ceux qui
  // ne passent pas par cette fonction.
}

// L'icône de sélection de la barre vivait ici — `syncBarBatchToggle` et `installBarBatchRemount`,
// retirées le 2026-08-26 avec l'emplacement `#sift-tb-actions-right` qu'elles servaient. Le
// commutateur du mode Lot est descendu dans la tête de la colonne de file, en bouton texte
// (`syncQueueSelectButton`, queue-panel.ts) : il vit désormais dans le même rendu que la file,
// donc le `MutationObserver` sur le titre de la barre — qui existait faute de point d'accroche sur
// le rendu de Revue depuis ce fichier — n'a plus d'objet. Retrait, pas mise en commentaire.

// `refreshRootGate` vivait ici (lecture de `library_root` + peinture du bandeau `#sift-gate`).
// Retirée le 2026-09-02 avec la porte elle-même (issue #54) ; sa lecture et sa décision sont
// passées dans `rail-root-warning.ts::refreshRootWarning`, appelée au même endroit du câblage et
// aussi par les deux points qui POSENT une racine (Réglages, popover de destination) — ce que
// l'ancienne, relue au seul démarrage, ne faisait pas.

async function refresh() {
  // La section Sources du rail remplace l'écran Accueil (fusion 1) : elle porte les mêmes comptes
  // de fichiers en attente, donc elle se rafraîchit exactement où l'écran Accueil le faisait.
  await renderRailSources();
  await renderQueue();
}

export function installLiveWiring() {
  registerBatchRenderer(renderBatch);
  registerRefreshHook(refresh);
  // Les huit globales `window.__sift*` ont disparu avec la maquette (étape 1, DESIGN.md § 17) :
  // `router.ts` appelle les renderers de vue directement, par import. app.js était leur unique
  // appelant, et elle ne tourne plus dans Tauri.
  injectLeanStyle();
  void injectTitlebar();
  void initTheme();
  installUndoShortcut();
  installFilingKeys();
  installQueueNavKeys();
  installScrollAutohide();
  installNavKeyboard();
  installRailToggle();
  installRailSources();
  // CTA « Ajouter un dossier à surveiller » de l'état vide de Revue (issue #53) : même geste que
  // le bouton du rail, même onChange — injecté ici parce que filing.ts ne peut pas importer
  // rail-sources sans refermer le cycle rail-sources → queue-panel → filing.
  registerAddSourceAction(() => {
    void pickAndAddFolder(async () => {
      await renderRailSources();
      void renderQueue(false);
    });
  });
  // Le toast de blocage « aucune racine » doit MENER à Réglages (issue #54). `filing-actions.ts`
  // ne peut pas importer `router.ts` (cycle statique router → queue-panel → filing →
  // filing-actions) : la navigation est injectée d'ici, comme le CTA d'ajout de source ci-dessus.
  registerOpenSettingsAction(() => goTo("reglages"));
  void refreshRootWarning();
  installWindowShortcuts();
  void installDragDrop();

  // Nav "Clé USB" (`data-view="cle"`) needs no special handling: app.js's own router renders the
  // screen and `window.__siftCle` above swaps in the live content. It used to be intercepted here
  // in the capture phase and redirected to Réglages, where the format card then lived — the nav
  // item lit up "Réglages" and landed on a page about something else. Both the interception and the
  // card moved out on 2026-07-31 (`usb-view.ts`).

  // `reanalyzeTrack` vivait ici en closure — remontée dans `queue-panel.ts` le 2026-08-26, où
  // vivent déjà ses deux moitiés d'état (`beginReanalyze` / `endReanalyze`) et d'où le menu
  // contextuel de la file l'appelle aussi. Importée depuis ce module, plus redéfinie.

  // Racine de dispatch : `document`, plus `#pa`. La barre unifiée (`#sift-titlebar`) est le
  // premier enfant de `<body>`, donc HORS de `#pa` — un écouteur enraciné sur `#pa` ne voit aucun
  // contrôle qu'une vue monte dans la barre. Chaque branche ci-dessous filtre déjà par `closest()`
  // sur son propre sélecteur, donc élargir la racine n'élargit pas ce qui est attrapé.
  document.addEventListener("click", (e) => {
    // Reanalyze-this-track button on an unanalysed queue row — checked BEFORE the .qi row-open
    // branch below (the button lives inside a .qi row) so clicking it never also opens the track.
    const reanalyzeBtn = (e.target as HTMLElement).closest<HTMLElement>("[data-reanalyze]");
    if (reanalyzeBtn) {
      e.stopPropagation();
      const id = Number(reanalyzeBtn.dataset.reanalyze);
      if (id) void reanalyzeTrack(id);
      return;
    }
    // Batch checkbox — handled by queue-panel.ts's own document listener; skip .qi dispatch
    if ((e.target as HTMLElement).dataset.sift === "queuepick") return;
    // queue item → open the live filing pane (report + editor + actions) in #mid
    const qi = (e.target as HTMLElement).closest<HTMLElement>(".qi[data-id]");
    if (qi?.dataset.id) {
      handleQueueItemClick(qi, e);
      // Cliquer une ligne pendant que le mode Lot est armé veut dire « inspecter celle-ci » :
      // `handleQueueItemClick` retombe alors en Détail par `enterDetailMode()`, sans passer par
      // `setReviewMode`. Ce chemin contournait le commutateur et laissait l'icône de barre armée à
      // tort — d'où une repeinture explicite ici jusqu'au 2026-08-26. Elle n'a plus lieu d'être :
      // `enterDetailMode` repeint le bouton lui-même, donc le contournement n'en est plus un.
      return;
    }
    // Le « masquer pour la session » de la porte de racine manquante (`[data-gate="dismiss"]`) a
    // été retiré avec la porte, le 2026-09-02 (issue #54). Le rappel du rail n'a rien à masquer :
    // il est déjà logé et tient sur deux lignes.
    // Rekordbox : choix d'une section dans la colonne de gauche (étape 10).
    const rkbSec = (e.target as HTMLElement).closest<HTMLElement>('[data-rkb="section"]');
    if (rkbSec?.dataset.sec) {
      e.stopPropagation();
      onRekordboxSectionPick(rkbSec.dataset.sec);
      return;
    }
    // Réglages : choix d'une catégorie dans la colonne de gauche (étape 9).
    const cat = (e.target as HTMLElement).closest<HTMLElement>('[data-reglages="cat"]');
    if (cat?.dataset.cat) {
      e.stopPropagation();
      onSettingsCategoryPick(cat.dataset.cat);
      return;
    }
    // Écartés actions (copy query / send-to-bin / restore / empty bin)
    const ec = (e.target as HTMLElement).closest<HTMLElement>("[data-ec]");
    if (ec) {
      e.stopPropagation();
      const act = ec.dataset.ec;
      const id = Number(ec.dataset.id);
      if (act === "copy-query") {
        // Retour par TOAST (`filing-toast.ts::copyToClipboard`), plus par une coche écrite dans le
        // bouton : celui-ci peut avoir été détaché entre le clic et la résolution — toute action
        // Écartés (corbeille, restauration) et tout changement d'écran reconstruisent `#content` par
        // `innerHTML` —, et le succès partait alors dans le vide en silence. La règle qui a motivé
        // ce site tient toujours et vit maintenant dans l'aide partagée : rien n'est peint AVANT que
        // l'écriture ait réussi, et un refus du presse-papier se dit.
        copyToClipboard(ec.dataset.q || "", "Recherche copiée");
      } else if (act === "trash") {
        void trashTrack(id)
          .then(renderEcartes)
          .catch((err) => {
            console.error("trash failed", err);
            toast("Échec : impossible d'envoyer à la corbeille");
          });
      } else if (act === "restore") {
        void restoreTrack(id)
          .then(renderEcartes)
          .catch((err) => {
            console.error("restore failed", err);
            toast("Échec : restauration impossible");
          });
      } else if (act === "requeue") {
        void requeueTrack(id)
          .then(renderEcartes)
          .catch((err) => {
            console.error("requeue failed", err);
            toast("Échec : remise en file impossible");
          });
      } else if (act === "purge") {
        void confirmAction(
          "Purger définitivement la corbeille ? Cette action est irréversible.",
          "Purger",
        ).then((ok) => {
          if (!ok) return;
          void purgeTrash()
            .then((res) => {
              // Files that resisted deletion (typically held open by another program) keep
              // their track in the bin. Staying silent would read as "the purge worked and
              // some tracks came back" — the one reading the user cannot act on.
              if (res.failed.length) {
                const s = res.failed.length > 1 ? "s" : "";
                toast(
                  `${res.purged} supprimé${res.purged > 1 ? "s" : ""} — ${res.failed.length} fichier${s} impossible${s} à supprimer (ouvert dans un autre programme ?)`,
                );
              }
              return renderEcartes();
            })
            .catch((err) => {
              console.error("purge failed", err);
              toast("Échec : purge de la corbeille impossible");
            });
        });
      } else if (act === "store") {
        void openUrl(decodeURIComponent(ec.dataset.url || "")).catch((err) =>
          console.error("open_url failed", err),
        );
      }
      return;
    }
    // Bibliothèque actions (quality chips / facet toggle / folder|genre pick / Discogs link / play)
    const bibEl = (e.target as HTMLElement).closest<HTMLElement>("[data-bib]");
    if (bibEl) {
      const act = bibEl.dataset.bib;
      if (act === "stat") {
        const stat = bibEl.dataset.stat;
        if (stat === "all") {
          bibState.filter.quality = undefined;
          bibState.filter.verdict = undefined;
          bibState.filter.q = undefined;
          bibState.filter.folder = undefined;
          bibState.filter.genre = undefined;
          bibState.filter.artist = undefined;
        } else if (stat === "lossless" || stat === "mp3") {
          bibState.filter.quality = stat;
          bibState.filter.verdict = undefined;
        } else if (stat === "duplicates") {
          bibDup.shown = !bibDup.shown;
          // Relance aussi après une erreur : sans `|| bibDup.error`, un scan échoué laissait
          // l'écran bloqué sur son message, le chip ne rejouant jamais rien.
          if (bibDup.shown && (bibDup.groups === null || bibDup.error)) {
            loadDuplicates();
            return;
          }
        } else if (stat === "fake") {
          bibState.filter.quality = undefined;
          bibState.filter.verdict = "fake";
        }
        void renderBiblioLive();
        return;
      } else if (act === "rkblink") {
        void (async () => {
          try {
            let defaultPath: string | undefined;
            try {
              const current = await rekordboxStatus();
              if (current.path) defaultPath = await dirname(current.path);
            } catch (e) {
              console.error("rekordbox_status failed (defaultPath lookup)", e);
            }
            const chosen = await openFolderDialog({
              multiple: false,
              directory: false,
              defaultPath,
              filters: [{ name: "Rekordbox XML", extensions: ["xml"] }],
            });
            if (!chosen || Array.isArray(chosen)) return;
            const status = await linkRekordboxXml(chosen);
            // Impasse A16 (issue #15) : cette branche testait `status.error`, que
            // `link_rekordbox_xml_inner` construit TOUJOURS à `None` — il échoue en `Err` avant de
            // renvoyer quoi que ce soit. Elle était donc inatteignable, et la seule chose qu'elle
            // pouvait dire vivait déjà dans le `catch`. Retirée plutôt que réparée : un succès qui
            // porterait une erreur n'existe pas dans ce contrat.
            toast(
              `XML Rekordbox lié : ${status.track_count} pistes, ${status.playlist_count} playlists`,
            );
            void renderRekordboxLive();
          } catch (e) {
            // Seconde moitié d'A16 : trois erreurs backend distinctes — fichier illisible
            // (`ipc_library.rs`, « lecture impossible »), XML invalide (`rekordbox_xml.rs`), et
            // échec d'écriture du réglage — devenaient un seul « Liaison échouée » qui ne disait à
            // l'utilisateur ni ce qui n'allait pas, ni s'il devait choisir un autre fichier. Les
            // messages backend sont déjà en français et déjà précis : les afficher suffit, sans
            // table de correspondance.
            toast(humanizeError(e, `Liaison du XML Rekordbox échouée : ${String(e)}`, "link_rekordbox_xml"));
          }
        })();
        return;
      } else if (act === "qual") {
        const q = bibEl.dataset.q;
        bibState.filter.quality = q === "all" ? undefined : (q as "lossless" | "mp3");
        // "Tous" doit réellement tout montrer — sans ce reset, un filtre verdict=fake posé via le
        // stat-card "À re-sourcer" restait actif indéfiniment (cul-de-sac trouvé à l'audit 2026-07-09).
        if (q === "all") bibState.filter.verdict = undefined;
        void renderBiblioLive();
      } else if (act === "facetpop") {
        e.stopPropagation(); // sinon la fermeture au clic-dehors (plus bas) le rouvre aussitôt
        toggleFacetPopover();
      } else if (act === "facet") {
        // Les TROIS valeurs, pas deux : « Artistes » retombait sur `folder` depuis l'ajout du
        // troisième onglet — cliquer Artistes sélectionnait Dossiers, en silence.
        const f = bibEl.dataset.f;
        bibState.facet = f === "genre" ? "genre" : f === "artist" ? "artist" : "folder";
        keepFacetPopoverOpen(); // ce rendu vient du menu : il est le seul à le rouvrir
        // Plus de bascule en place : le type de facette est un item de MENU depuis le 2026-08-19,
        // et un menu se referme sur le rendu suivant de toute façon. La classe `.on` qu'on togglait
        // ici appartenait au contrôle segmenté supprimé.
        void renderBiblioLive();
      } else if (act === "viewmode") {
        bibState.viewMode = bibEl.dataset.mode === "grid" ? "grid" : "table";
        document
          .querySelectorAll<HTMLElement>("#sift-bib-viewmode-seg [data-bib='viewmode']")
          .forEach((b) => b.classList.toggle("on", b.dataset.mode === bibState.viewMode));
        positionViewModeThumb();
        void renderBiblioLive();
      } else if (act === "sort") {
        // Un en-tête est à la fois un bouton de tri et la poignée de déplacement de sa colonne.
        // Quand le geste s'est terminé en déplacement, le `click` arrive quand même ici — et trier
        // en plus de réordonner ferait deux choses pour un seul geste.
        if (consumeSortSuppression()) return;
        const field = bibEl.dataset.field as LibrarySortState["field"];
        bibState.sort =
          bibState.sort.field === field
            ? { field, dir: bibState.sort.dir === "asc" ? "desc" : "asc" }
            : { field, dir: "asc" };
        void renderBiblioLive();
      } else if (act === "pick") {
        closeFacetPopover(); // une valeur choisie ferme le sélecteur ; changer d'ONGLET le garde
        const key = bibEl.dataset.key as "folder" | "genre" | "artist";
        const val = bibEl.dataset.val;
        // toggle off if re-clicking the active facet value
        const cur =
          key === "folder" ? bibState.filter.folder : key === "genre" ? bibState.filter.genre : bibState.filter.artist;
        const next = cur === val ? undefined : val;
        bibState.filter.folder = key === "folder" ? next : undefined;
        bibState.filter.genre = key === "genre" ? next : undefined;
        bibState.filter.artist = key === "artist" ? next : undefined;
        void renderBiblioLive();
      } else if (act === "link") {
        const rid = bibEl.dataset.rid;
        if (rid) void openUrl(`https://www.discogs.com/release/${rid}`);
      } else if (act === "row") {
        // SÉLECTION (étape 5). Clic simple : sélectionne et ouvre. ⇧+clic : étend la plage.
        // ⌘/Ctrl+clic : ajoute ou retire. Les deux modificateurs ne touchent PAS au détail ouvert —
        // étendre une sélection n'est pas ouvrir une piste, et rouvrir à chaque touche de plage
        // lancerait autant de chargements de rapport qu'il y a de lignes traversées.
        const id = Number(bibEl.dataset.id);
        const ordered = sortTracks(bibState.tracks, bibState.sort).map((t) => t.id);
        applyRowClick(id, { shift: e.shiftKey, meta: e.metaKey || e.ctrlKey }, ordered);
        if (e.shiftKey || e.metaKey || e.ctrlKey) {
          void renderBiblioLive().then(renderSelectionSummary);
        } else {
          // Clic simple : marquer la ligne EN PLACE plutôt que rebuilder. Même motif que le `.cur`
          // juste en dessous — la liste est virtualisée, les lignes hors fenêtre n'existent pas
          // dans le DOM, et `bibSelection` les couvre au remontage. Sans ce marquage immédiat, la
          // sélection existait en état sans rien peindre : mesuré à 0 ligne `.sel` après un clic
          // simple, alors que ⇧+clic en montrait bien trois.
          // `paintBibSelection` plutôt que deux `classList` locales depuis le 2026-08-19 : le clic
          // droit repeint par la même fonction, et elle tient aussi `aria-selected`. Deux
          // marquages côte à côte auraient divergé sur ce que le lecteur d'écran annonce.
          paintBibSelection();
          openBiblioDetail(id);
        }
      } else if (act === "play" || act === "identify" || act === "tile") {
        // Open the unified detail/edit panel (report + inline editor + identify + actions).
        openBiblioDetail(Number(bibEl.dataset.id));
      } else if (act === "dupscan") {
        bibDup.shown = !bibDup.shown;
        if (bibDup.shown && (bibDup.groups === null || bibDup.error)) {
          loadDuplicates();
        } else {
          void renderBiblioLive();
        }
      } else if (act === "dupretry") {
        // Bouton du bloc d'erreur : relance sans rebasculer l'affichage.
        loadDuplicates();
      } else if (act === "dupresolve") {
        const idx = Number(bibEl.dataset.idx);
        const group = bibDup.groups?.[idx];
        if (!group) return;
        const losers = group.members.filter((m) => !m.recommend_keep).map((m) => m.id);
        void confirmAction(
          `Envoyer ${losers.length} doublon${losers.length > 1 ? "s" : ""} à la corbeille ? Le morceau recommandé est conservé.`,
          "Envoyer à la corbeille",
        ).then((ok) => {
          if (!ok) return;
          // `Promise.all` rejette au PREMIER échec : sur 5 doublons dont un seul refuse, les 4
          // autres partaient bien à la corbeille et l'écran annonçait « impossible d'envoyer les
          // doublons », en gardant le groupe intact. L'utilisateur relançait donc une suppression
          // sur des pistes déjà supprimées, qui échoue à `trash_file_fs` — impasse.
          //
          // `renderBiblioLive()` ne RESCANNE PAS les doublons : il repeint la section depuis
          // `bibDup.groups` en mémoire. Rafraîchir ne suffit donc pas — il faut retirer du groupe
          // les membres réellement supprimés, sinon ils restent listés avec leur bouton et la
          // boucle recommence. Un groupe qui retombe sous deux membres n'est plus un groupe.
          void Promise.allSettled(losers.map((id) => trashTrack(id)))
            .then((results) => {
              const groups = bibDup.groups || [];
              const failedIds = new Set(
                losers.filter((_, i) => results[i]?.status === "rejected"),
              );
              if (failedIds.size === 0) {
                bibDup.groups = groups.filter((_, i) => i !== idx);
                return renderBiblioLive();
              }
              results.forEach((r, i) => {
                if (r.status === "rejected") {
                  console.error(`dupresolve: trashTrack(${losers[i]}) failed`, r.reason);
                }
              });
              const g = groups[idx];
              if (g) {
                g.members = g.members.filter((m) => m.recommend_keep || failedIds.has(m.id));
                if (g.members.length < 2) bibDup.groups = groups.filter((_, i) => i !== idx);
              }
              const done = losers.length - failedIds.size;
              toast(
                done === 0
                  ? "Aucun doublon n'a pu être envoyé à la corbeille"
                  : `${done} doublon${done > 1 ? "s" : ""} envoyé${done > 1 ? "s" : ""} à la corbeille, ${failedIds.size} en échec`,
              );
              return renderBiblioLive();
            })
            .catch((e: unknown) => {
              console.error("dupresolve: refresh failed", e);
              toast("Échec : impossible de rafraîchir la liste");
            });
        });
      }
      return;
    }
    const el = (e.target as HTMLElement).closest<HTMLElement>("[data-sift]");
    if (!el) return;
    const act = el.dataset.sift;
    if (act === "reviewmode") {
      e.stopPropagation();
      setReviewMode(el.dataset.m === "batch" ? "batch" : "detail");
    } else if (act === "batchqueuefile") {
      e.stopPropagation();
      void handleBatchQueueAction("file");
    } else if (act === "batchqueuediscard") {
      e.stopPropagation();
      void handleBatchQueueAction("discard");
    } else if (handleBatchAction(el, act ?? "", e)) {
      // handled — see batch-panel.ts
    } else if (handleRekordboxAction(el, act ?? "", e, () => void runNavExport())) {
      // handled — see rekordbox-view.ts
    }
  });

  // "File in place" checkbox (under the #fldz tree, batch mode) — a checkbox, so it needs change.
  // MENU CONTEXTUEL de la table (étape 5). Enraciné sur `document` pour la même raison que le
  // clic : la barre unifiée vit hors de `#pa`, et une entrée de rail y viendra un jour.
  //
  // Il ne retire PAS d'un coup les boutons de la ligne : le bouton lecture reste, c'est le geste
  // primaire, et les deux affordances restantes (identifier, fiche Discogs) sont doublées ici. Le
  // menu est ce qui permettra de les sortir de la ligne sans perdre l'action.
  // Double-clic sur une ligne = ouvrir l'emplacement du fichier (`docs/ui-specs/bibliotheque.md`
  // § Interactions). Enraciné sur `document` comme le clic droit, et pas sur `#pa` : la table peut
  // vivre hors de ce conteneur selon l'écran.
  document.addEventListener("dblclick", (e) => {
    const row = (e.target as HTMLElement).closest<HTMLElement>('.lr[data-bib="row"]');
    if (!row?.dataset.id) return;
    // Le premier clic du double a déjà ouvert le détail (dispatch de `click`) — c'est voulu, les
    // deux gestes se complètent : on regarde la piste, puis on va voir son fichier.
    void revealTrack(Number(row.dataset.id)).catch((err: unknown) =>
      toast(humanizeError(err, "Impossible d'ouvrir l'emplacement", "reveal_track")),
    );
  });

  installFacetPopoverDismiss();

  // Fermeture du sélecteur de facette au clic dehors. Sur `document` et non sur `#pa` : un clic
  // dans le rail ou dans la barre unifiée doit le fermer aussi, et ces deux-là vivent hors de `#pa`.
  document.addEventListener("click", (e) => {
    const t = e.target as HTMLElement;
    if (t.closest(".sift-facet-pop") || t.closest('[data-bib="facetpop"]')) return;
    closeFacetPopover();
  });

  document.addEventListener("contextmenu", (e) => {
    // L'en-tête d'abord : il porte ses propres réglages (colonnes), pas les actions d'une piste.
    if ((e.target as HTMLElement).closest(".sift-lib-thead")) {
      e.preventDefault();
      openColumnHeaderMenu(e.clientX, e.clientY);
      return;
    }
    const row = (e.target as HTMLElement).closest<HTMLElement>('.lr[data-bib="row"]');
    if (!row?.dataset.id) return;
    e.preventDefault();
    // Le menu et ses actions vivent dans `bibliotheque-view.ts`, avec la sélection sur laquelle
    // ils portent : depuis les actions de masse (2026-08-19), ouvrir ce menu peut CHANGER la
    // sélection, et cet état n'a jamais habité ici.
    openBiblioContextMenu(e.clientX, e.clientY, Number(row.dataset.id));
  });

  document.addEventListener("change", (e) => {
    const ip = (e.target as HTMLElement).closest<HTMLInputElement>('input[data-sift="inplace"]');
    if (ip) onBatchInPlaceChange(ip.checked);
  });

  // queue:changed fires per burst source (watcher debounce window) AND periodically during a
  // large folder scan (scanner.rs emits it every 25 net-changed files) — debounce the redraw
  // the same way onAnalysisChanged does below, so a fast burst of pings coalesces into one.
  let queueChangeTimer: ReturnType<typeof setTimeout> | undefined;
  void onQueueChanged(() => {
    clearTimeout(queueChangeTimer);
    queueChangeTimer = setTimeout(() => void refresh(), 150);
  });
  // Impasse A4 (issue #15) : un scan de dossier surveillé qui n'a pas tourné laissait sa source
  // en « À jour » vert, indiscernable d'un dossier réellement à jour. La raison arrive en toast
  // (dite une fois, en toutes lettres) et l'état persiste sur la pastille de la source.
  // Fréquence : au plus un par ajout ou rescan de dossier — jamais en rafale, contrairement à
  // `queue:changed` juste au-dessus, qui est debouncé pour cette raison.
  void onScanFailed((sourceId, reason) => {
    noteScanFailure(sourceId, reason);
    toast(`Le scan du dossier surveillé a échoué : ${reason}`);
  });
  void onFileDone(onFileBatchDone);
  void onFileProgress(pushFileProgress);
  // Stop button on the global zone's "file" row → stop-net cancel of the running filing batch.
  setCancelHandler("file", onFileStop);

  // Analysis pings can arrive several times per second — debounce the queue redraw.
  let t: ReturnType<typeof setTimeout> | undefined;
  // Throttle the progress-zone IPC+render: coalesce bursts to one RAF per frame (~16 ms).
  // Events are never dropped — only renders are coalesced. A trailing 350 ms timeout
  // guarantees a final render once pings stop (catches the done==total transition).
  let pendingAnalyzeRender = false;
  let analyzeTrailTimer: ReturnType<typeof setTimeout> | undefined;
  function scheduleAnalyzeRender() {
    // Reset the trailing timer on every event so it fires only after silence.
    clearTimeout(analyzeTrailTimer);
    analyzeTrailTimer = setTimeout(() => void pushAnalyzeProgress(), 350);
    if (pendingAnalyzeRender) return;
    pendingAnalyzeRender = true;
    requestAnimationFrame(() => {
      pendingAnalyzeRender = false;
      void pushAnalyzeProgress();
    });
  }
  void onAnalysisChanged(() => {
    // A report may have changed (re-analysed / replaced file) → drop the in-session cache so
    // the next open re-fetches from the DB (the source of truth) instead of serving it stale.
    void import("./report-view").then((m) => m.clearReportCache());
    // Throttle progress-zone update: IPC + DOM render at most once per RAF frame (~16 ms),
    // not once per event (can be dozens per second during a 4000-track analysis burst).
    scheduleAnalyzeRender();
    clearTimeout(t);
    // touchDetail=false: redraw the queue list only; never re-open the open track (that aborts
    // the player's audio load).
    t = setTimeout(() => void renderQueue(false), 300);
  });

  // Catch an analysis already in flight when the app opens (events only fire on each item after).
  void pushAnalyzeProgress();
  void refresh();
}

