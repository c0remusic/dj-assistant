// Une seule chose, et elle est structurelle : rendre le message que l'appelant veut AFFICHER, en
// garantissant que la chaîne brute part en `console.error`. C'est tout.
//
// La garantie vit ici et pas dans la discipline des appelants parce que deux sites l'avaient déjà
// oubliée : les cartes de candidats de `filing-identify.ts` et `library-detail.ts` affichaient
// `esc(String(e))` sans le moindre log — la chaîne brute ne subsistait nulle part. Et le repo n'a
// aucun harnais de test frontend, donc la console est la seule preuve disponible après coup.
//
// Aucune table de correspondance code→message, DÉLIBÉRÉMENT. Une première version en portait une,
// devinée et non mesurée, et elle était fausse dans les deux sens : le seul producteur du littéral
// « db lock » est `ipc_filing.rs`, pour un `Mutex` EMPOISONNÉ que `db.rs::lock_conn` propage sans
// jamais tenter de récupérer — donc irrécupérable jusqu'au redémarrage, à qui elle répondait
// « réessaie dans un instant » ; pendant que le seul échec DB réellement transitoire (SQLITE_BUSY
// après le `busy_timeout`) remonte « database is locked », qu'elle ne matchait pas. Un sentinel
// `NoLibraryRoot` a aussi été essayé puis retiré : vérifié, il ne vient que de `file_track` /
// `file_batch` / `list_bins` / `create_bin`, qu'aucun appelant d'ici n'emprunte — c'était une
// branche morte.
//
// Les cas de domaine restent chez ceux qui savent les lire (`bibliotheque-view.ts`, `journal.ts`,
// `usb-format-modal.ts`, la limite de débit Discogs de `filing-identify.ts`) : ils sont PLUS précis
// que tout repli générique. Un appelant qui a un message spécifique le passe en `display` — la
// journalisation reste garantie sans qu'il perde sa précision.

/**
 * Journalise `e` et rend `display`.
 *
 * `display` est le message de l'appelant, pas un passe-partout : il doit nommer CE qui a échoué.
 * Un humanisateur qui rendrait « une erreur est survenue » remplacerait une chaîne illisible par
 * une chaîne inutile. Quand l'appelant distingue plusieurs cas, il calcule `display` lui-même et
 * passe par ici dans TOUS les cas — c'est ce qui garantit qu'aucune branche ne reste muette.
 *
 * `context` étiquette la ligne de log : passer l'opération (`"revert"`, `"undoLast"`).
 */
export function humanizeError(e: unknown, display: string, context?: string): string {
  const raw = e instanceof Error ? e.message : String(e);
  console.error(context ? `[${context}] ${raw}` : raw, e);
  return display;
}
