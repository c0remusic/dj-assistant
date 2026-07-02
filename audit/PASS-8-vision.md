# PASS 8 — Vision produit

> Synthèse transversale des PASS 0-7. Aucune nouvelle exploration de code, aucune
> modification de fichier. Chaque constat référence le rapport source précis.
> Méthode : 5 angles de lecture successifs sur les preuves déjà établies, sévérité
> pré-commercialisation.

---

## Ce qui donne une impression premium

- **Le verdict qualité est pédagogique, pas opaque.** `report-view.ts` affiche un
  kbps estimé en langage DJ ("MP3 ≈ 192 kbps"), une légende explicite et un
  spectrogramme annoté avec la ligne de coupure dessinée dessus — cf.
  PASS-6-produit.md, Constat 2. C'est le point le plus fort du produit : le
  mécanisme de détection est *montré*, pas juste affirmé.
- **Le parcours nominal est réellement sans friction.** Accueil → watcher
  automatique → Revue son-d'abord → SPACE/ENTER/BKSP. Aucune étape obligatoire
  parasite, Discogs et tags ID3 sont explicitement optionnels avant rangement,
  les seules confirmations modales protègent des actions bulk irréversibles —
  cf. PASS-6-produit.md, Constat 5. C'est un flux "un geste par morceau" qui tient
  sa promesse.
- **Le moteur d'analyse est un deep module exemplaire** (décode → accumulateurs
  purs → verdict testé, interface étroite) et le filing est correctement
  découpé en 3 phases avec lock DB relâché autour de l'encode — cf.
  PASS-1-architecture.md, "Ce qui est bien conçu". Ce n'est pas visible pour
  l'utilisateur final mais conditionne la fiabilité perçue à terme (pas de gel
  UI pendant un encode, cf. aussi PASS-4-perfs.md a1).
- **Discipline de gestion d'erreurs Rust quasi irréprochable** : 0 `unwrap()`
  dangereux sur 454 occurrences brutes auditées (450 en tests, 4 au bootstrap
  acceptables) — cf. PASS-2-qualite.md §1. Un produit qui ne plante pas sur
  fichier malformé/réponse réseau/chemin exotique inspire confiance à l'usage,
  même si l'utilisateur ne le voit jamais directement.
- **Progression basée sur données réelles**, pas des barres factices : `progress-
  zone.ts` utilise done/total réels avec pattern DOM "create once, mutate" —
  cf. PASS-5-ui-ux.md M1 (nuance positive) et PASS-4-perfs.md (cité comme
  référence de bon pattern face à `renderQueue`).

## Ce qui trahit un prototype/amateur

- **Une feature de la nav principale est une simulation pure sans aucun
  backend.** "Rekordbox"/"Clé USB" déclenchent un `setInterval` de 450ms par
  piste qui fait avancer une barre de progression crédible (icône USB, compte
  réel de pistes via `listLibrary()`) — puis rien n'est écrit sur disque. Le
  code le dit lui-même : *"the work itself is simulated"* — cf.
  PASS-6-produit.md, Constat 3 (Critical). Rien dans l'UI ne distingue cette
  entrée de nav des fonctionnalités réelles.
- **Écran Bibliothèque en anglais avec styles inline pendant que tout le reste
  est en français avec des classes établies** — "Save"/"Delete"/"Identify" vs
  "Ranger"/"Jeter"/"Identifier" ailleurs, boutons construits en `style="..."`
  ad hoc au lieu des classes `.sift-*` — cf. PASS-5-ui-ux.md H3 (High). Un
  bouton "Delete" n'a même pas de fond visuel malgré la règle documentée
  d'aplat ghost pour les actions destructives.
- **Trois patterns visuels différents pour la même idée** ("action faite,
  annulable") : bouton-toggle pour Appliquer les tags, bannière séparée pour
  Ranger, toast éphémère 6s pour Écarter/Jeter — cf. PASS-5-ui-ux.md H2. Aucune
  règle perceptible pour deviner lequel s'applique où (violation Nielsen #4).
- **Zéro `aria-label` sur les boutons icon-only** des écrans Revue/Bibliothèque/
  Écartés/Journal, alors que le correctif a été appliqué une fois (nav
  Bibliothèque dans `sift-live.ts`) et jamais généralisé — cf. PASS-5-ui-ux.md
  C2 (Critical). Play/pause, Annuler, Fermer, Trash n'ont qu'un `title`
  (tooltip souris, pas fiable au clavier/lecteur d'écran).
- **Espacements hors grille documentée** dans `ecartes-view.ts` (`gap:7px` répété
  alors que la grille impose 4/8/12/16/24/32) et couleur "danger" réutilisée
  pour un état qui n'est pas une erreur (badge générique "à re-sourcer") — cf.
  PASS-5-ui-ux.md M3/M4. Détails mineurs isolément, mais visibles en changeant
  rapidement d'écran.
- **Un god file qui grossit au lieu d'être traité** : `sift-live.ts` est passé
  de 942 à 1412 lignes malgré le split déjà annoncé dans CLAUDE.md — cf.
  PASS-7-maintenabilite.md §2. La dette s'aggrave activement, ce n'est pas un
  point figé qu'on peut ignorer.

## Incohérences design ↔ comportement ↔ fonctionnalités les plus graves

Classées par risque de confiance utilisateur si découvertes en usage réel :

1. **[Le plus grave] Export Rekordbox/Clé USB simulé et présenté comme une
   fonctionnalité réelle** — cf. PASS-6-produit.md, Constat 3. Un DJ qui clique
   "Rekordbox" voit une progression crédible avec un vrai compte de pistes,
   puis rien ne se passe. Découverte au pire moment : au club, la veille d'un
   gig, quand il croit avoir exporté sa bibliothèque. C'est la seule incohérence
   qui peut casser une session DJ réelle, pas juste une frustration UI.
2. **Garde-fou anti-upscale contournable par une extension trompeuse
   (BUG-1 Critical)** — cf. PASS-3-bugs.md. `rail_from_ext` se fie à
   l'extension du fichier, pas au contenu réel déjà sondé par Symphonia. Un
   MP3 renommé `.flac` peut être "converti" en AIFF lossless techniquement
   valide mais vide de sens — exactement le scénario que le garde-fou existe
   pour bloquer. Grave parce que ça touche le cœur de la promesse produit
   (détection de faux lossless) au moment précis où elle devrait protéger
   l'utilisateur.
3. **Seuils cutoff→kbps divergents entre Rust (source de vérité du verdict) et
   TS (affichage)** — cf. PASS-1-architecture.md HIGH#3, PASS-2-qualite.md §4,
   PASS-7-maintenabilite.md §5. Un même fichier peut afficher un kbps estimé
   qui ne correspond pas au seuil ayant réellement produit le verdict. Grave
   parce que ça mine la crédibilité de la feature signature elle-même : un DJ
   technique qui remarque l'incohérence perd confiance dans TOUT le verdict,
   pas seulement dans l'affichage.
4. **Différenciateur produit réel (compat CDJ) noyé dans l'UI** — cf.
   PASS-6-produit.md, Constat 1. Le README vend la compat hardware CDJ comme
   LE argument face à Rekordbox/MediaMonkey, mais l'écran principal (Revue) ne
   mentionne jamais "CDJ" — juste une ligne oui/non sous Genres. Moins
   dangereux qu'un bug actif, mais stratégiquement grave : le produit ne
   raconte pas sa propre histoire à l'endroit où l'utilisateur passe le plus
   de temps.
5. **Identification Discogs sans jeton de session, risque de croisement de
   métadonnées entre morceaux (H1)** — cf. PASS-5-ui-ux.md. Contrairement à
   `openFilingInto` (protégé par `openSeq`), `doIdentify`/`onIdentityApplied`
   n'ont pas de garde de séquence — un DJ qui tape "I" puis "Enter" vite peut
   voir une identité Discogs s'appliquer silencieusement au mauvais morceau.
   Bug de données silencieux, pas de crash — donc difficile à détecter par
   l'utilisateur lui-même, ce qui aggrave le risque de confiance une fois
   découvert.

## Freins concrets à l'adoption/la commercialisation

- **Onboarding Discogs non trivial** : le token exige de créer une app
  développeur sur discogs.com — friction réelle pour un DJ non technique,
  cf. PASS-6-produit.md Constat 4.
- **Réglages exposés sans hiérarchie** (`filename_template`, `trash_purge_days`
  au même niveau que `library_root`/`discogs_token`) — surface cognitive inutile
  pour la quasi-totalité des DJ, cf. PASS-6-produit.md Constat 4.
- **Incohérence de langue et de style entre écrans** (Bibliothèque en anglais)
  — donne l'impression de deux applications collées ensemble, cf.
  PASS-5-ui-ux.md H3. C'est le genre de détail qui affecte la perception de
  "produit fini" plus que sa gravité technique ne le suggère.
- **Bug cross-disk latent sur la restauration corbeille** (rename au lieu de
  copy→verify) — cf. PASS-1-architecture.md HIGH#1. Invisible en dev (tout sur
  un disque), reproductible chez un utilisateur avec plusieurs volumes —
  configuration DJ courante (source sur disque externe, bibliothèque en local).
- **Filing conformant qui échoue cross-disk** (rename direct) — cf.
  PASS-1-architecture.md HIGH#2. Frappe justement les fichiers déjà propres
  (FLAC/AIFF/WAV) qu'on garde tels quels — cas d'usage fréquent.
- **Tests d'encodage qui passent silencieusement sans rien tester** si les
  fixtures sont absentes (`eprintln!("skip...")` sans échec visible) — cf.
  PASS-7-maintenabilite.md §3. Risque que le chemin d'encodage réel (cœur du
  produit : "déplacer = encoder + ranger") régresse sans qu'aucun CI ne le
  détecte avant la découverte utilisateur.

## Perception de vitesse et de réactivité (au-delà des perfs mesurées)

- **Point aveugle net** : entre le clic "Filer (n)" et le premier événement
  `file:progress`, l'utilisateur ne voit qu'un spinner générique sans
  pourcentage — potentiellement plusieurs secondes sur un gros premier fichier
  à encoder, cf. PASS-5-ui-ux.md M1. Contraste avec la barre 0/N qui pourrait
  apparaître immédiatement au clic.
- **Re-décodage invisible mais réel au clic spectrogramme** : chaque ouverture
  du spectrogramme en Revue redéclenche un décodage PCM complet même sur un
  fichier déjà entièrement analysé et caché — 200-700ms de délai perçu par
  clic, cf. PASS-4-perfs.md b2 (High). Le cache `report_json` a été conçu
  explicitement pour éviter ce re-décodage, mais son scope exclut le
  spectrogramme — donc l'utilisateur perçoit une latence sur une action qui
  *devrait* être instantanée vu l'existence du cache.
- **Risque de jank pendant un scan initial de grosse bibliothèque** :
  `renderQueue` reconstruit tout le DOM de la liste (`innerHTML=`) à chaque
  tick d'analyse (débounce 300ms), potentiellement des dizaines de fois par
  minute sur un scan de plusieurs milliers de fichiers — cf. PASS-4-perfs.md
  d1 (High), en violation directe de la règle CLAUDE.md "créer une fois, muter
  ensuite". Le point aveugle est documenté par les auteurs eux-mêmes en
  commentaire ("dozens per second during a 4000-track analysis burst") mais
  jamais corrigé — signal que le problème est connu, pas juste théorique.
- **Contrepoint positif** : le worker d'analyse ne bloque jamais l'UI (décodage
  hors lock DB, parallélisme réel `available_parallelism().clamp(1,4)`), et le
  filing relâche le lock autour de l'encode FFmpeg — donc la fluidité
  *structurelle* de l'app pendant les opérations lourdes est bonne ; les points
  faibles sont localisés (spectrogramme, liste de queue), pas systémiques.

## Verdict par angle

**Lead/Staff Engineer** — La base tiendrait une croissance modérée mais pas
sans investissement de dette d'abord. Le moteur Rust (analysis/, filing.rs en
3 phases, migrations additives testées) est du code de niveau senior, prêt à
accueillir des features. Le vrai risque est le double-miroir Rust↔TS
(règles métier, seuils, contrats — cf. PASS-1 HIGH#3, PASS-2 §4, PASS-7 §5)
qui transforme chaque évolution de règle backend en risque de dérive
silencieuse côté front, et le front lui-même (`sift-live.ts`, `filing.ts`,
plus de 3000 lignes à eux deux, 0 framework de test JS trouvé) qui accumule
la complexité au lieu de la répartir. Un troisième développeur qui rejoindrait
l'équipe aujourd'hui mettrait plus de temps à comprendre "quelle règle vit où"
que la taille du code ne le suggère.

**Senior Product Designer** — L'expérience trahit clairement un produit encore
en construction, pas un prototype jetable, mais pas un produit fini non plus.
Le noyau (écran Revue, verdict pédagogique, parcours son-d'abord) est soigné
et cohérent — c'est visiblement la partie qui a reçu une vraie passe de
design. Mais dès qu'on sort de ce noyau (Bibliothèque en anglais avec styles
inline, 3 patterns de confirmation différents, zéro aria-label généralisé),
l'impression bascule vers "assemblage de features développées à des moments
différents sans repasse de cohérence". Un œil non technique verrait d'abord le
bon (le player, le verdict) ; un œil exigeant verrait vite les coutures.

**UX Lead** — Le produit *paraît* globalement réactif sur le chemin nominal
(écouter → ranger, feedback immédiat), mais deux trous cassent cette
impression dès qu'on sort du chemin optimal : le spectrogramme qui recharge à
chaque clic malgré un cache censé l'éviter, et l'absence de retour immédiat
au lancement d'un batch de filing. Ce ne sont pas des lenteurs réelles
(PASS-4 confirme que l'architecture ne bloque jamais l'UI structurellement)
mais des trous de *perception* — l'utilisateur ne sait pas si son clic a
été pris en compte pendant une fenêtre de plusieurs centaines de ms à
quelques secondes, ce qui est exactement le type de détail qui définit "ça a
l'air rapide" indépendamment de la vitesse réelle.

**Tech Lead** — La plus dangereuse des incohérences, de loin, est l'export
Rekordbox/Clé USB simulé exposé sans garde-fou dans la nav principale — ce
n'est pas un défaut de polish, c'est une fonctionnalité qui ment activement
sur son propre état à l'utilisateur, dans un rail de navigation qu'il verra à
chaque session. Juste derrière, le contournement du garde-fou anti-upscale par
extension trompeuse (BUG-1) est plus grave techniquement mais moins visible
au quotidien. Les deux partagent un point commun : ils touchent directement
la proposition de valeur du produit (fiabilité de rangement, promesse
d'export) au moment où l'utilisateur en a le plus besoin — pas des bugs
cosmétiques.

**DJ pro (utilisateur expert)** — Le différenciateur CDJ est réel et
techniquement solide (détection de faux lossless avec preuve visuelle), mais
tel qu'exposé aujourd'hui (ligne secondaire sous Genres, jamais nommé dans le
verdict principal), rien ne le distingue au premier regard d'un détecteur
générique — un DJ pressé ne comprendra pas pourquoi changer d'outil sans lire
la doc. Les frictions UI (Bibliothèque en anglais, incohérence de feedback)
seraient agaçantes mais pas rédhibitoires en usage courant. En revanche,
l'export Rekordbox simulé serait rédhibitoire et dangereux s'il était
découvert la veille d'un gig — un DJ qui compte dessus pour préparer sa clé
USB et découvre sur place que rien n'a été exporté ne rouvrira jamais
l'application. C'est le seul point de cet audit qui peut transformer un
utilisateur convaincu en utilisateur qui prévient activement ses pairs de ne
pas utiliser l'outil.
