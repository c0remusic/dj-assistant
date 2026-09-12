// Jeton de génération de rendu — le garde-fou qui empêche un rendu d'écran ARRIVÉ EN RETARD de
// peindre par-dessus l'écran courant (issue #42, « L'écran courant change tout seul pendant un
// scan de source »).
//
// Chaque renderer de vue (`renderBiblioLive`, `renderRekordboxLive`, `renderEcartes`,
// `renderReglagesLive`, `loadAndPaint` du Journal) capture `#content` en tête, PUIS attend un ou
// plusieurs allers-retours IPC, PUIS écrit son `innerHTML`. Or `#content` n'est jamais remplacé par
// une navigation — `blockShell`/`revueShell` (`router.ts`) ne font que réassigner son contenu —
// donc la référence capturée reste vivante indéfiniment. Sans jeton, un rendu parti pour l'écran A
// qui revient après un passage sur l'écran B écrit A dans le `#content` de B : l'écran change tout
// seul, sans qu'aucun clic de navigation n'ait eu lieu.
//
// Pourquoi le symptôme ne se voit que sous scan : le backend est synchrone derrière un
// `Mutex<Connection>` unique (CLAUDE.md § Backend). Un scan tient ce verrou en rafale, donc chaque
// aller-retour IPC passe de quelques millisecondes à plusieurs secondes. La fenêtre de course
// s'ouvre de « en pratique jamais » à « plusieurs secondes » — l'ordre de grandeur relevé dans le
// ticket (« ~3 s plus tard »).
//
// SANS DOM ET SANS IMPORT, délibérément, pour deux raisons :
//   1. `router.ts` importe les modules de vue ; un module de vue qui importerait `router.ts` en
//      retour ferait un cycle statique, interdit par CLAUDE.md § Modules frontend (les splits
//      passent par injection de dépendance, jamais par un import retour). Ce module est une
//      feuille : tout le monde peut l'importer sans rien refermer.
//   2. C'est de la logique pure, donc gelable par Vitest en env Node (`test/view-epoch.test.ts`).
//
// CE QUE LE JETON NE COUVRE PAS, et il faut le lire avec le reste : il distingue les GÉNÉRATIONS,
// pas les appels. Deux `renderBiblioLive()` concurrents lancés SANS navigation entre eux (frappe de
// recherche rapide, clics de facette enchaînés) portent le même jeton et écriront tous les deux —
// dernier arrivé, dernier peint. C'est une course intra-écran distincte, non traitée ici.

/** Génération courante. Monotone croissante ; jamais remise à zéro — un jeton périmé doit le rester
 *  pour toute la session, sinon un rendu très en retard redeviendrait valide par recyclage. */
let epoch = 0;

/** Ouvre une nouvelle génération. Appelé par `render()` (`router.ts`) AVANT de déléguer à la vue,
 *  pour que le renderer qui démarre capture le jeton neuf et non celui qu'il périme. */
export function bumpViewEpoch(): number {
  return ++epoch;
}

/** La génération courante. À capturer en tête de renderer, dans le même geste que `#content` —
 *  les deux vont ensemble : la référence au nœud, et le droit d'y écrire. */
export function viewEpoch(): number {
  return epoch;
}

/** `true` quand `token` n'est plus la génération courante : l'écran a changé pendant l'attente, et
 *  ce rendu ne doit plus rien peindre. À tester après CHAQUE `await` qui précède une écriture. */
export function isStaleViewRender(token: number): boolean {
  return token !== epoch;
}
