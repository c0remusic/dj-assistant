// Rendu d'une entrée de disque amovible de la colonne B′ de l'écran Clé USB, extrait de
// `usb-view.ts` pour deux raisons : la story Storybook doit appeler la MÊME fonction que l'app
// (sinon elle documente une copie qui dérive), et ce module n'importe que `dom.ts`, `usage-chart.ts`
// (pur) plus un type — donc l'importer n'entraîne aucun code Tauri, contrairement à `usb-view.ts`
// qui tire `ipc.ts`.
//
// Jusqu'au 2026-09-09 la fonction rendait une LIGNE (`usbRowHtml`, lettre · modèle · taille · bouton
// Formater…) dans une carte de 560 px. Déclinaison #24, sixième écran (`docs/ui-specs/cle-usb.md`
// § Décision 2026-09-09) : l'écran est lu contre Utilitaire de disque, et un disque y est une
// ENTRÉE DE SIDEBAR — glyphe, nom, capacité à droite — la zone C portant le reste. Même grammaire
// `.fld` que la colonne des sections de Rekordbox.
import type { RemovableDrive } from "./ipc";
import { esc } from "./dom";
import { formatGo } from "./usage-chart";

/** Ce que l'utilisateur appelle ce disque. `drive.id` est devenu un chemin de disque physique
 * (`\\.\PHYSICALDRIVE2`, `/dev/disk4`) le 2026-07-31 pour que les clés non formatées puissent
 * exister dans la liste — correct côté backend, mais illisible sur une ligne et impossible à
 * retaper dans une confirmation. La lettre d'abord ; un disque non monté n'en a pas, donc son
 * numéro. */
export function driveDisplayName(drive: RemovableDrive): string {
  if (drive.mount) return drive.mount;
  const n = /(?:PHYSICALDRIVE|disk)(\d+)/i.exec(drive.id)?.[1];
  return n ? `Disque ${n}` : drive.id;
}

/** Une entrée de la colonne des disques. Un lecteur énuméré mais vide (`has_media: false`) garde
 * sa lettre dans l'explorateur Windows et n'a pourtant rien à formater : il est listé ET dit
 * « vide », plutôt que masqué — c'est cette contradiction (« aucun disque détecté » alors que
 * l'explorateur montre un lecteur USB) qui a coûté la soirée du 2026-07-31. Le détail (modèle,
 * système de fichiers, « aucun média inséré ») vit en zone C, sur la tête et la grille de faits.
 *
 * `data-usb-id` porte l'identifiant opaque du disque, jamais interprété ici. Toute donnée non
 * fiable passe par `esc()`. */
export function usbEntryHtml(drive: RemovableDrive, on: boolean): string {
  return (
    `<div class="fld${on ? " on" : ""}" data-usb-id="${esc(drive.id)}" tabindex="0" role="button" aria-pressed="${on}">` +
    '<i class="ti ti-usb" aria-hidden="true"></i>' +
    `<span class="sift-usb-entry-name">${esc(driveDisplayName(drive))}</span>` +
    `<span class="rkb-entry-count">${drive.has_media ? formatGo(drive.size_bytes) : "vide"}</span>` +
    "</div>"
  );
}
