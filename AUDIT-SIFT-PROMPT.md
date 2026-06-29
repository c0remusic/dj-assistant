# AUDIT SIFT — Mission

Tu réalises un audit complet et approfondi du projet Sift, considéré comme
un produit professionnel destiné à la commercialisation (outil de
préparation / analyse DJ pro ; référence implicite = MAO et outils DJ haut
de gamme, sans chercher à les copier).

═══════════════════════════════════════════════════════════
RÈGLES ABSOLUES (lis-les avant tout)
═══════════════════════════════════════════════════════════

1. AUDIT SEULEMENT. Tu ne modifies, ne crées, ne corriges AUCUN fichier
   source. Seul livrable autorisé : les fichiers `.md` dans `audit/`.

2. MÉTHODE DÉTECTIVE. Crime → théorie → preuve → (jamais de fix ici).
   Chaque problème DOIT citer `fichier:ligne` et inclure l'extrait de code
   concerné. Pas de preuve grep-able = pas de problème listé. Si tu
   soupçonnes sans pouvoir prouver dans le code, classe-le en section
   séparée « HYPOTHÈSES NON VÉRIFIÉES » avec « comment le vérifier ».

3. N'ANALYSE RIEN QUE TU N'AIES OUVERT. Aucune conclusion sur un fichier
   non lu, aucune supposition d'architecture non parcourue.

4. SOIS SÉVÈRE. Le but est d'améliorer le produit, pas de valider
   l'existant. Pas de complaisance.

5. PAS DE BLOAT. Aucun refactoring pour la seule élégance. Chaque reco doit
   apporter un bénéfice concret : stabilité, perf, simplicité,
   maintenabilité, évolutivité, UX, qualité perçue. 10 problèmes prouvés
   valent mieux que 40 supposés. Fusionne les redites entre passes.

6. PERFORMANCE = OBJECTIF PRODUIT. La vitesse est un critère premier.
   À chaque pass, signale tout ce qui ralentit conversion, analyse,
   spectrogramme ou navigation. Une lenteur perceptible = défaut au même
   titre qu'un bug.

7. PRINCIPES DE CODE À FAIRE RESPECTER (signale toute violation) :
   fail-fast, pas de fallback silencieux, une seule voie correcte, pas de
   backup implicite, séparation des responsabilités, changements
   chirurgicaux. Une violation de ces principes EST un problème à lister.

═══════════════════════════════════════════════════════════
ÉTAPE 0 — CARTE DU PROJET (obligatoire, avant toute pass)
═══════════════════════════════════════════════════════════

- Arborescence complète : modules Rust (commandes Tauri, analyse, verdict,
  conversion) vs front (Vite/TS, composants, état, vues).
- Points d'entrée, frontière Rust ↔ front (commandes invoke, events).
- Schéma SQLite (tables, index, migrations).
- Intégrations : FFmpeg (conversion), Symphonia (décodage), lofty
  (métadonnées), génération spectrogramme.
- Fichiers les plus volumineux / concentrant le plus de logique.
- Pour CHAQUE pass à venir, liste d'abord les fichiers que tu liras.

Écris `audit/PASS-0-carte.md`.

═══════════════════════════════════════════════════════════
GESTION DU CONTEXTE
═══════════════════════════════════════════════════════════

Écris le rapport de CHAQUE pass dans `audit/PASS-N-nom.md` DÈS qu'elle est
finie. Ne garde pas tout en mémoire ; le rapport final relit ces fichiers.
Cela évite que les dernières passes soient bâclées par saturation.

═══════════════════════════════════════════════════════════
FORMAT D'UN PROBLÈME
═══════════════════════════════════════════════════════════

- Priorité : Critical / High / Medium / Low
- Fichier : `chemin:ligne`
- Composant / fonction
- Extrait de code (preuve)
- Description
- Impact concret
- Cause probable
- Proposition de correction (description, PAS d'application)
- Effort : Faible / Moyen / Élevé
- Bénéfice attendu

═══════════════════════════════════════════════════════════
LES PASSES
═══════════════════════════════════════════════════════════

→ PASS 1 — Architecture
  Globale, organisation, séparation des responsabilités, modularité,
  couplage, dette, extensibilité, cohérence.
  Questions ciblées Sift :
  - La frontière Rust ↔ front est-elle nette, ou de la logique métier
    fuit-elle côté TS (ou de l'UI côté Rust) ?
  - Combien de commandes Tauri ; font-elles une seule chose chacune, ou
    sont-elles des god-commands ?
  - L'accès SQLite est-il centralisé (une couche) ou dispersé ?
  - L'état front est-il unique et traçable, ou plusieurs sources de vérité
    (état local + DB + cache) peuvent-elles diverger ?
  - Ajouter un nouveau type d'analyse / critère de filing touche combien de
    fichiers ? (mesure de couplage)
  Écris `audit/PASS-1-architecture.md`.

→ PASS 2 — Qualité du code
  Lisibilité, nommage, duplication, fonctions trop longues, complexité,
  cohérence de style, robustesse, gestion d'erreurs, code mort/inutile.
  Questions ciblées Sift :
  - Gestion d'erreurs Rust : `Result` propagé (`?`) ou `unwrap()`/`expect()`
    pouvant paniquer en prod ? Liste chaque `unwrap` sur un chemin runtime.
  - Fallbacks silencieux (valeur par défaut au lieu d'une erreur) qui
    violent fail-fast ? Pointe-les.
  - Plusieurs façons de faire la même chose (deux chemins de conversion,
    deux lectures de tags) ? Violation « une voie ».
  - Duplication Rust ↔ TS (constantes, seuils — ex. seuils de verdict par
    bitrate dupliqués) ?
  - Code mort : commandes Tauri jamais appelées, branches jamais atteintes,
    champs DB jamais lus.
  Écris `audit/PASS-2-qualite.md`.

→ PASS 3 — Bugs potentiels
  null/undefined, états incohérents, race conditions, erreurs silencieuses,
  edge cases, variables jamais mises à jour, asynchronisme, régressions.
  Zéro supposition (sinon → hypothèses non vérifiées).
  Questions ciblées Sift :
  - Races : l'analyse en arrière-plan peut-elle écrire en DB pendant qu'une
    autre opération lit/modifie le même track ? Le filing peut-il tourner
    pendant l'analyse du même fichier ?
  - SQLite concurrent : connexions partagées, transactions, verrous, risque
    « database is locked » ?
  - Sentinel `"__SOURCE__"` (filer sur place) : tous les chemins le gèrent-ils,
    ou certains le traitent comme un vrai chemin ?
  - Edge cases fichiers : corrompu, extension trompeuse, tags absents, durée
    nulle, fichier déplacé/supprimé entre analyse et conversion.
  - États UI désynchronisés de la DB après revert / batch partiel.
  - Events Tauri arrivant après démontage du composant écouteur ?
  Écris `audit/PASS-3-bugs.md`.

→ PASS 4 — Performances (axe produit prioritaire)
  Méthode détective stricte : ne déclare pas un chemin « lent » sans preuve
  dans le code. Localise le coût (boucle, allocation, copie, appel bloquant,
  re-render) ET propose comment le mesurer. Distingue « coût prouvé par le
  code » de « à confirmer par mesure runtime ».

  a) CONVERSION (FFmpeg)
     - Copie vs ré-encodage inutile ? Batch parallèle ou série ? Conversions
       redondantes ? Fichier chargé entier en mémoire vs streaming ? I/O ?
  b) ANALYSE (Symphonia / lofty / verdict)
     - Décodage full-file vs fenêtres ? Mêmes données décodées plusieurs
       fois ? Fichier lu plusieurs fois (tags + analyse séparés) ? L'analyse
       sature-t-elle le CPU au point de figer l'UI ? Thread/pool dédié ?
  c) SPECTROGRAMME
     - FFT : taille de fenêtre, recalcul à chaque rendu vs cache ? Thread
       principal vs worker ? Résolution générée >> affichée ? Recalculé
       inutilement sur resize / changement de track / re-render ? Canvas vs
       GPU ?
  d) FLUIDITÉ UI (navigation menus)
     - Re-renders inutiles ? Listes virtualisées pour gros catalogues ?
       Requêtes SQLite synchrones sur le thread UI ? État rechargé à chaque
       navigation au lieu d'être caché ? Memoization absente ?

  Pour chaque optim : effort/bénéfice, gain attendu, faisabilité sans casser
  fail-fast / no-fallback.
  Écris `audit/PASS-4-perfs.md`.

→ PASS 5 — UI / UX
  Cohérence visuelle, hiérarchie, espacement, alignements, densité,
  lisibilité, navigation, ergonomie, feedback, états des contrôles,
  accessibilité si pertinent. Signale les écrans confus/surchargés.
  Questions ciblées Sift :
  - Le workflow de filing donne-t-il un feedback clair à chaque étape
    (banner, état Filed, revert) ou l'utilisateur doute-t-il du résultat ?
  - Contrôles : boutons désactivés pendant une opération longue, ou
    cliquables et source d'incohérence ?
  - Densité du rail de détail : lisible et hiérarchisé, ou surchargé ?
  - Feedback pendant conversion/analyse : progression réelle ou spinner
    opaque ?
  - Cohérence : mêmes actions présentées de la même façon partout ?
  Écris `audit/PASS-5-ui-ux.md`.

→ PASS 6 — Logique produit
  Fonctionnalités peu claires/redondantes, options inutiles, complexité
  excessive, friction, workflow, cohérence des interactions.
  Question systématique : « Cette fonctionnalité apporte-t-elle vraiment de
  la valeur ? »
  Questions ciblées Sift :
  - Le différenciateur réel (compatibilité hardware CDJ : 32-bit float,
    header EXTENSIBLE, E-8305) est-il mis en avant, ou noyé parmi des
    features secondaires ?
  - Options de config que l'utilisateur ne comprendra pas / ne touchera
    jamais ?
  - Parcours « importer → analyser → filer » : étapes de friction évitables ?
  - Fonctionnalités qui se recouvrent (deux façons de faire la même chose) ?
  Écris `audit/PASS-6-produit.md`.

→ PASS 7 — Maintenabilité
  Évolution, risques futurs, architecture long terme, zones fragiles,
  dépendances, découpage, testabilité.
  Questions ciblées Sift :
  - Zones les plus fragiles (un changement y casse souvent autre chose) ?
    Prouve par le couplage observé.
  - Dépendances : versions figées FFmpeg/Symphonia/lofty, risque de rupture
    à la mise à jour ? Surface d'API large ou réduite ?
  - Logique critique (verdict, conversion, filing) testable en isolation, ou
    trop couplée I/O + UI ?
  - Migrations SQLite versionnées et sûres ?
  - Constantes/seuils centralisés ou éparpillés (risque d'oubli) ?
  Écris `audit/PASS-7-maintenabilite.md`.

→ PASS 8 — Vision produit
  Angles : Lead/Staff Engineer, Senior Product Designer, UX Lead, Tech Lead,
  utilisateur expert (DJ pro). Détermine :
  - ce qui donne une impression premium ;
  - ce qui fait prototype/amateur ;
  - les incohérences design ↔ comportement ↔ fonctionnalités ;
  - les freins à l'adoption / la commercialisation ;
  - la perception de vitesse : le produit *paraît*-il rapide et réactif
    (feedback immédiat, pas de gel), au-delà des perfs réelles ?
  Écris `audit/PASS-8-vision.md`.

→ PASS 9 — Benchmark concurrentiel & opportunités

  PRÉREQUIS : recherche web activée. Si elle ne l'est pas, ÉCRIS-LE en tête
  du fichier et ne produis QUE la colonne « Sift » + la liste des questions
  à rechercher plus tard — n'invente AUCUNE feature concurrente de mémoire.

  Règle anti-hallucination : chaque feature attribuée à un concurrent doit
  citer une source (URL / page produit). Pas de source = « à vérifier », pas
  une affirmation.

  Catégories de comparaison :
  - Préparation de bibliothèque DJ : Rekordbox, Engine DJ, Serato, Lexicon,
    VirtualDJ.
  - Analyse qualité audio / détection de faux : Mixed In Key (Integrity),
    Platinum Notes, fakin' the funk, Spek.
  - Spectrogramme / inspection : Spek, Sonic Visualiser, iZotope RX.
  - Conversion / batch : dBpoweramp, XLD, fre:ac.

  Pour chaque catégorie, un tableau :
  | Capacité | Sift (preuve code, réf. PASS 1-8) | Concurrents (source) | Verdict |
  Verdict ∈ {Sift fait mieux, parité, Sift en retard, Sift unique}.
  La colonne Sift s'appuie sur les PASS-1→8 (référence fichier), pas sur une
  supposition.

  Puis :
  a) CE QUE LES CONCURRENTS FONT MIEUX
     (qui, quoi, pourquoi ça compte pour un DJ pro, source)
  b) CE QUE LES CONCURRENTS ONT EN PLUS
     (features absentes de Sift mais standard ailleurs)
  c) OPPORTUNITÉS — propositions de fonctions / améliorations. Pour chacune :
     - Description
     - Problème utilisateur résolu
     - Alignée avec le différenciateur Sift (compat CDJ) ? oui/non
     - Effort (Faible/Moyen/Élevé) du point de vue du code actuel
     - Bénéfice (adoption / qualité perçue / différenciation)
     - Risque de bloat (cf. règle 5) : mérite-t-elle d'exister, ou dilue-t-elle
       le produit ?

  GARDE-FOU : Sift a un différenciateur clair (compatibilité CDJ). Ne propose
  pas d'en faire un clone de Rekordbox. Priorise ce qui renforce le
  différenciateur ou retire de la friction, pas l'ajout de features « parce
  que les autres les ont ».

  Écris `audit/PASS-9-benchmark.md`.

═══════════════════════════════════════════════════════════
RAPPORT FINAL — `audit/RAPPORT-FINAL.md`
═══════════════════════════════════════════════════════════

Relis les PASS-N.md puis produis :

## Notes (/10) — sois sévère
Ancrage : 7/10 = niveau commercial atteint. Justifie CHAQUE note par 2-3
faits du code (référence fichier).
- Architecture, Qualité du code, Performances, Maintenabilité, UI, UX,
  Cohérence produit, Qualité perçue, Niveau de finition, Préparation
  commerciale.

## Note Performance détaillée (/10 par axe — sois sévère)
- Conversion / Analyse / Spectrogramme / Fluidité UI.
Pour chaque axe : goulot principal prouvé dans le code + gain le plus
rentable.

## Forces / Faiblesses du projet

## Positionnement marché
- Le différenciateur unique de Sift est-il réel et défendable ? (preuve)
- Top 5 opportunités à fort levier (Pass 9), classées effort/bénéfice,
  filtrées anti-bloat.
- Une ligne : pourquoi un DJ pro choisirait Sift plutôt que son outil actuel.

## Top 20 des améliorations (max 20, classées par rapport effort/bénéfice)

## Roadmap (ordre optimal d'implémentation)
1. Corrections critiques
2. Quick Wins
3. Améliorations importantes
4. Optimisations à long terme

═══════════════════════════════════════════════════════════
VÉRIFICATION FINALE
═══════════════════════════════════════════════════════════

Dernière passe transversale : détecte les problèmes/incohérences manqués par
les passes individuelles (notamment ce qui ne se voit qu'en croisant
plusieurs modules). Ajoute-les au rapport.

Rappel : aucun fichier source modifié. Tout vit dans `audit/`.
