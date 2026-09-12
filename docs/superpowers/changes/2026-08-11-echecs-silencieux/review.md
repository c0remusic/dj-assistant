# Échecs silencieux — exécution et vérification

Date d'exécution : **2026-08-17**, session sans supervision (Antoine dormait). Le chantier était
ouvert depuis le 2026-08-11 avec son seul `design.md` — pas de `plan.md`, pas d'exécution.

Le `design.md` posait une règle que ce document respecte : **13 des 21 impasses avaient été
établies par lecture de code d'un agent, sans relecture indépendante**, avec la consigne de
« re-vérifier chacune contre le code courant avant de la corriger — un constat daté n'est pas un
fait présent ». Chaque ligne ci-dessous dit donc d'abord ce qui a été **relu**, et où.

## Ce qui a été corrigé

Quatre commits, du plus grave au plus périphérique.

| # | Impasse | Correctif | Fichiers |
|---|---|---|---|
| 1 | **A17** `list_journal` avale ses erreurs | signature `-> rusqlite::Result<Vec<_>>` | `actions.rs`, `ipc_filing.rs`, `journal.ts` |
| 1 | **A21** thème confirmé mais non enregistré | `setTheme` rend le résultat de la persistance | `theme.ts`, `reglages-view.ts` |
| 1 | **A3** 4 actions Accueil muettes | toast + `humanizeError` | `home-sources.ts` |
| 1 | **A7** spinner perpétuel du rail Revue | carte d'erreur + Réessayer, ou toast | `queue-panel.ts` |
| 1 | **A18** Journal en page blanche | `paintJournal()` rattrape les 3 `void render…()` | `journal.ts`, `sift-live.ts` |
| 1 | **A19** graphique d'occupation disparu | slot gardé, chaîne brute, Réessayer | `bibliotheque-view.ts` |
| 1 | **A20** carte d'erreur sans porte de sortie | bouton Réessayer | `bibliotheque-view.ts` |
| 2 | **A13** Rekordbox « à jour » sans `master.db` | `RekordboxLinkStatus.masterdb_error` | `ipc_library.rs`, `contracts.ts`, `rekordbox-view.ts` |
| 2 | **A14** en-tête « à jour » sur 4 cartes en erreur | 3e état « n'a pas répondu » | `rekordbox-view.ts` |
| 2 | **A15** « réessaie plus tard » sur du permanent | cause nommée quand elle est connue | `rekordbox-view.ts` |
| 2 | **A10** jeton refusé lu « injoignable » | `ProviderError::BadToken` (401/403) | `metadata/mod.rs`, `discogs.rs`, `identify-shared.ts` |
| 2 | **A9** désactivation décrite en dégradation | texte Réglages + message d'exécution | `reglages-view.ts`, `identify-shared.ts` |
| 2 | **A11** (moitié) débounce jamais vidé | flush au `blur` | `reglages-view.ts` |
| 3 | **A2** conseil faux sur FFmpeg absent | branche `spawn failed` avant les autres | `filing-actions.ts` |
| 3 | **A4** badge vert « À jour » permanent | événement `scan:failed` + état « Scan en échec » | `ipc.rs`, `ipc.ts`, `home-sources.ts`, `sift-live.ts` |
| 3 | **A5** « Rien d'importable » pour un réglage absent | `ImportResult.blocked_by` | `ipc.rs`, `ipc.ts`, `chrome.ts` |
| 3 | **A8** « racine non choisie » sur un échec de lecture | `destState.loadError`, séparé de `rootSet` | `filing-bins.ts` |
| 3 | **A16** branche morte + 3 causes aplaties | branche retirée, causes affichées | `sift-live.ts` |
| 4 | **A6** Revue vide sans action | bouton vers Accueil via `actionHtml` | `filing.ts` |
| 4 | **A12** aucune détection de Rekordbox | `is_dir()` dans `resolve_pioneer_dir` | `rekordbox_repairs.rs` |

**Le modèle a été suivi, pas réinventé** : `usb-view.ts` — le seul écran qui distinguait déjà
correctement l'absence de l'échec — est la référence de forme pour A19, et le rail jumeau
`home-sources.ts` pour A7. Aucun de ces deux fichiers n'a été touché.

## Ce que la relecture a confirmé, et ce qu'elle a démenti

**Confirmées ligne à ligne contre le code du 2026-08-17** : A2, A3, A5, A7, A8, A9, A10, A12, A13,
A14, A16, A17, A18, A19, A20, A21. Les huit marquées ✅ dans `design.md` l'étaient déjà ; les huit
autres venaient de la lecture d'agent et tenaient toutes.

**Une correction au `design.md` :** il affirme que `read_masterdb_index` écrase ses erreurs
« sans log » (`actions.rs:233-246`). C'est faux — la fonction fait bien un `log::error!` avant de
rendre `None` (`actions.rs`, `read_masterdb_index`). Le défaut réel n'est pas l'absence de trace
mais que la trace soit **côté serveur** : le front reçoit `None` et ne peut rien en dire. C'est
`rekordbox_repairs.rs::read_masterdb_path_map` qui, lui, aplatit ses trois causes avec deux `.ok()?`
et **aucun** log — le reproche vaut là, pas à l'autre endroit.

**Un désaccord assumé, non corrigé :** A16 compte « l'annulation du sélecteur de fichier → `return`
sans rien dire » comme une impasse. Ce n'en est pas une. Annuler une boîte de dialogue est une
action délibérée de l'utilisateur, dont le silence **est** le résultat attendu, sur toutes les
plateformes. Un message y serait du bruit. Les deux autres moitiés d'A16 (branche morte, causes
aplaties) sont bien des défauts et sont corrigées.

## Les tests, et ce qu'ils tiennent réellement

Trois tests Rust ajoutés. Chacun porte un **contrôle positif** dans le même corps, pour la raison
exacte que ce chantier corrige : sans lui, un test qui n'observe qu'un `Err` ne distingue pas une
fonction qui échoue au bon moment d'une fonction qui échoue toujours — il rejouerait dans son
propre corps la confusion « vide vs cassé ».

- `actions::journal_surfaces_a_broken_schema_instead_of_reading_empty` — **mesuré par mutation**,
  comme l'exige `CLAUDE.md` § Méthode. Remettre `Ok(Vec::new())` sur l'échec de `prepare` le fait
  tomber sur `une table absente doit rendre Err, jamais un Vec vide` ; le correctif restauré le
  repasse au vert. Les deux exécutions ont eu lieu, ce n'est pas une déduction.
- `ipc_library::masterdb_error_distinguishes_a_missing_master_db_from_a_present_one` — les trois
  branches (rien de lié / absent / présent) dans le même test, parce qu'un `Some` seul ne prouve
  pas qu'un champ discrimine quoi que ce soit.
- `rekordbox_repairs::applying_names_a_missing_pioneer_folder_instead_of_failing_later`.

**Ce que ces tests NE couvrent pas**, et il faut le lire tel quel : aucun ne s'exécute dans la
vraie fenêtre. Tous les correctifs frontend — dix-sept des vingt lignes du tableau — sont vérifiés
par `tsc`, ESLint et la relecture. La suite Vitest tourne en env Node sans DOM par construction
(`CLAUDE.md` § Architecture), donc elle ne pouvait pas les couvrir non plus.

## Passe de non-régression dans la vraie fenêtre — 2026-08-17, `run-sift`

Faite. Elle prouve que rien n'est **cassé** ; elle ne prouve presque rien sur les correctifs
eux-mêmes, et la distinction est le point de cette section.

Identité de la session vérifiée avant toute mesure (`document.title` = `Sift — prépa sons DJ`), et
**binaire Rust confirmé à jour** par une sonde qui ne pouvait répondre que sur le nouveau code :
`rekordbox_status` rend bien la clé `masterdb_error`. Sans ça, un `tauri dev` de 8 s aurait pu
servir un binaire d'avant les correctifs.

- **Les 8 écrans du rail peignent**, avec un compte positif chacun — `#content` non vide, texte de
  130 à 3038 caractères. Aucun `[]` ne sert de preuve ici.
- **Zéro `console.error`, zéro `onerror`, zéro `unhandledrejection`** sur le parcours complet, avec
  les trois collecteurs installés AVANT la navigation.
- Ouverture d'une piste réelle : 45 lignes de file, `#mid` peint, 2 zones. Le chemin Revue complet
  (waveform, verdict, pied d'action) est intact — c'est `filing.ts` que A6 a touché.
- Plancher typographique : `nbTexts: 166`, `minPx: 10`, `under10: []` — le compte positif rend ce
  `[]` lisible.
- Capture de Revue prise et regardée.

**Deux correctifs seulement ont été VUS s'exécuter** :

- **A9** — le texte de Réglages lu dans le DOM est bien le nouveau (« Sans jeton, Sift n'interroge
  pas Discogs du tout : le bouton Identifier renvoie ici »).
- **A6** indirectement : l'écran Revue se comporte normalement, mais la file d'Antoine contient
  2709 morceaux, donc **l'état vide ne s'est pas rendu** et le bouton ajouté n'a pas été vu.

**A13/A14/A15 n'ont PAS été exercés**, et c'est un résultat, pas un oubli : aucun XML Rekordbox
n'est lié sur cette machine (`rekordbox_status` rend `linked: false`), donc l'écran affiche son
état « Aucun XML Rekordbox lié » et les quatre cartes de synchronisation **ainsi que la ligne
d'en-tête que A14 modifie ne sont jamais construites**. Le résultat vide de la mesure a été
diagnostiqué en dumpant le texte de l'écran, pas compté comme une réussite.

Tous les autres correctifs se déclenchent sur un ÉCHEC, qu'aucun parcours normal ne produit : ils
restent non observés. Le protocole ci-dessous est ce qu'il faudrait faire pour les voir.

## Portée exacte de `masterdb_error`, à ne pas élargir en le lisant

Le champ répond à **« le fichier est-il là, à l'endroit où on le cherche »**. Il ne dit pas qu'il
se déchiffre : le vérifier coûterait le déchiffrement SQLCipher multi-Mo à chaque affichage de
l'écran Rekordbox, et cet appel est un statut. Un `master.db` **présent mais illisible** reste donc
un `None` muet chez les détecteurs, et l'écran affichera « à jour ».

C'est un choix, pas un oubli : le cas visé par l'inventaire est la machine SANS Rekordbox, où le
fichier est absent. Le cas « présent mais corrompu » demande une autre décision (payer le
déchiffrement, ou le mettre en cache) et n'a pas été tranché ici.

## Ce qui reste ouvert

- **A1 — aucun parcours de premier lancement.** Hors chantier par construction : c'est la décision
  [#16](https://github.com/c0remusic/sift/issues/16) de la map. La concevoir ici la
  court-circuiterait.
- **A11, moitié « Vérifier ».** Le jeton n'est toujours pas validé à la saisie, et
  « Jeton enregistré. » s'affiche pour n'importe quelle chaîne. Le libellé est **exact** — il dit
  l'écriture, pas la validité — et A10 défuse le pire de la combinaison : un jeton refusé se dit
  maintenant comme tel au premier Identifier, au lieu de passer pour une panne réseau. Ce qui
  manque est un bouton « Vérifier », donc une **commande IPC neuve et une décision de surface** :
  pas une correction de défaut localisé, et pas quelque chose à poser sans Antoine.
- **La lisibilité de `master.db`**, voir ci-dessus.
- **Les 7 vérifications sur profil vierge** listées par #15 restent écartées (décision du
  2026-08-11, inchangée) : elles demandent de déplacer la vraie base de l'utilisateur, et aucun
  correctif de ce document ne les attendait.
- **La vérification dans la vraie fenêtre.** C'est le trou principal de cette session. Le protocole
  est le skill `run-sift` ; les états à voir sont listés ci-dessous.

## Protocole de vérification visuelle — à exécuter, pas encore fait

Chaque ligne demande de PROVOQUER l'échec, ce qu'aucun parcours normal ne fait. Par ordre de coût :

1. **A4** — ajouter un dossier surveillé pointant sur un chemin qu'on supprime aussitôt : la
   pastille doit passer « Scan en échec » rouge, et un toast doit nommer la cause.
2. **A5** — vider `library_root` dans Réglages, puis déposer un dossier sur « Où on va » : le toast
   doit parler du réglage, pas du contenu déposé.
3. **A13/A14/A15** — lier un XML Rekordbox valide, renommer le dossier `Pioneer/rekordbox` : les
   quatre cartes doivent dire « indisponible », l'en-tête ne doit plus dire « à jour », et le
   message de section doit nommer `master.db` au lieu de conseiller d'attendre.
4. **A9/A10** — coller un jeton Discogs bidon, cliquer Identifier : « Discogs a refusé le jeton »,
   pas « injoignable ».
5. **A2** — renommer le binaire `ffmpeg` du bundle, puis Ranger : le message doit nommer FFmpeg et
   ne plus dire « Réessaie ».
6. **A17/A18** — le plus coûteux, demande une base illisible. À faire sur une COPIE.

## Gates passées

À chaque commit : `tsc --noEmit`, `npm run test` (30 Vitest), `npm run lint`, `npm run lint:tokens`,
`cargo fmt --check`, `cargo clippy --all-targets -D warnings`, `cargo test` (549 + 9 après ajouts).
