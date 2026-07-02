# RAPPORT FINAL — Audit Sift

> Synthèse des 9 passes d'audit (`audit/PASS-0-carte.md` à `audit/PASS-9-benchmark.md`),
> lues intégralement pour ce rapport. Sévérité pré-commercialisation, ancrage constant
> `main` du 2026-07-02, branche `m6a-discogs`. Aucun fichier source modifié — tout
> vit dans `audit/`.

---

## Notes (/10) — sois sévère

Ancrage : 7/10 = niveau commercial atteint.

| Axe | Note | Justification (2-3 faits) |
|---|---|---|
| **Architecture** | 6.5/10 | Moteur d'analyse deep module exemplaire, filing 3-phases avec lock relâché (PASS-1 "Ce qui est bien conçu"). Mais double-miroir Rust↔TS non testé (PASS-1 HIGH#3, MEDIUM#2) et une primitive d'inversion dupliquée avec bug cross-disk réel (PASS-1 HIGH#1). |
| **Qualité du code** | 7/10 | 0 `unwrap()`/`expect()` dangereux sur 454 occurrences auditées côté Rust (PASS-2 §1) — discipline rare. Mais duplication de seuils cutoff→kbps avec valeurs *divergentes* Rust/TS (PASS-2 §4) et `installLiveWiring` à 259 lignes mêlant 6 responsabilités (PASS-2 §6). |
| **Performances** | 5.5/10 | Structure saine (pas de fichier en mémoire, lock jamais tenu pendant un encode, worker parallèle réel — PASS-4 a1/b4). Mais le spectrogramme ignore systématiquement son propre cache (PASS-4 b2, High) et `renderQueue` viole la règle CLAUDE.md "create once, mutate" en rafale (PASS-4 d1, High). |
| **Maintenabilité** | 6/10 | Migrations SQLite exemplaires (additives, versionnées, 10 tests — PASS-7 §4). Mais `sift-live.ts` a grossi de 942 à 1412 lignes malgré un split annoncé (PASS-7 §2, High), et les tests d'`encode.rs` passent silencieusement sans rien tester si les fixtures manquent (PASS-7 §3, High). |
| **UI** | 5.5/10 | Écran Revue soigné et cohérent (PASS-5, PASS-8 "impression premium"). Mais zéro `aria-label` généralisé (PASS-5 C2, Critical), Bibliothèque en anglais avec styles inline pendant que le reste est en français avec classes établies (PASS-5 H3, High). |
| **UX** | 6/10 | Parcours nominal réellement sans friction (PASS-6 Constat 5). Mais 3 patterns visuels différents pour "action faite, annulable" (PASS-5 H2) et un point aveugle de feedback entre le clic "Filer" et le premier `file:progress` (PASS-5 M1). |
| **Cohérence produit** | 4.5/10 | Le vrai différenciateur (compat CDJ) est noyé sous Genres, jamais nommé dans le verdict principal (PASS-6 Constat 1, PASS-8). Une entrée de la nav principale ("Rekordbox"/"Clé USB") simule une fonctionnalité qui n'existe pas (PASS-6 Constat 3, Critical). |
| **Qualité perçue** | 5/10 | Le noyau (Revue, verdict pédagogique avec kbps estimé et spectrogramme annoté) est premium (PASS-8). Mais dès qu'on en sort, l'impression bascule vers "assemblage de features développées à des moments différents" (PASS-8, verdict Senior Product Designer). |
| **Niveau de finition** | 4.5/10 | Export simulé exposé sans garde-fou dans la nav (PASS-6 Constat 3), garde-fou anti-upscale contournable par une extension trompeuse sur la feature signature elle-même (PASS-3 BUG-1, Critical). |
| **Préparation commerciale** | 4/10 | Deux défauts Critical touchent directement la confiance utilisateur au pire moment (gig, feature signature) — cf. Positionnement marché ci-dessous. Pas prêt tel quel, mais les corrections sont majoritairement à effort Faible/Moyen. |

## Note Performance détaillée (/10 par axe — sois sévère)

| Axe | Note | Goulot principal prouvé | Gain le plus rentable |
|---|---|---|---|
| **Conversion** | 7/10 | Batch de filing 100% série, un seul thread FFmpeg actif à la fois (PASS-4 a2, Medium, gain non quantifié) | Paralléliser la phase 2 (`execute_file`, déjà hors lock DB) sur un pool borné à 2-3 threads |
| **Analyse** | 6/10 | Un même fichier est décodé jusqu'à 2-4 fois par cycle complet (analyse + fingerprint séparés, PASS-4 b1, High) | Fusionner l'ouverture analyse+fingerprint quand les deux sont demandés dans le même cycle |
| **Spectrogramme** | 4/10 | `analyze_path(with_spectrogram=true)` ignore TOUJOURS le cache `report_json`, re-décode tout à chaque clic (PASS-4 b2, High) — le goulot le plus significatif de toute la passe perf | Étendre `report_json` pour inclure le spectrogramme (déjà borné à 204 800 octets max) — effort Faible, élimine 100% des re-décodages |
| **Fluidité UI** | 5.5/10 | `renderQueue` fait `innerHTML=` sur toute la liste à chaque tick d'analyse en rafale, violation directe de la règle CLAUDE.md (PASS-4 d1, High) | Appliquer le pattern "create once, mutate" déjà exemplaire dans `progress-zone.ts` du même repo |

## Forces / Faiblesses du projet

**Forces**
- Moteur d'analyse Rust : deep module pur, testé, zéro `unwrap()` dangereux sur 454 occurrences (PASS-1, PASS-2).
- Filing en 3 phases (plan/execute/commit) avec lock DB relâché autour de l'encode — pas de gel UI structurel (PASS-1, PASS-4 a1).
- Verdict qualité pédagogique et visuellement prouvé (kbps estimé, spectrogramme annoté) — point fort UX unique confirmé par le benchmark concurrentiel (PASS-6 Constat 2, PASS-9 Catégorie 2).
- Migrations SQLite additives, versionnées, testées (PASS-0, PASS-7 §4).
- Parcours nominal sans friction parasite : écouter → ranger/écarter en un geste (PASS-6 Constat 5).
- Différenciateur CDJ techniquement réel et sans équivalent chez aucun concurrent audité, y compris les convertisseurs généralistes (PASS-9 Catégorie 4).

**Faiblesses**
- Feature "Export Rekordbox/Clé USB" exposée dans la nav principale = pure simulation sans backend (PASS-6 Constat 3, Critical).
- Garde-fou anti-upscale contournable par une extension de fichier trompeuse, sur la feature signature (PASS-3 BUG-1, Critical).
- Double-miroir manuel Rust↔TS (règles métier, seuils cutoff→kbps divergents) sans test de contrat (PASS-1, PASS-2, PASS-7).
- Zéro `aria-label` généralisé sur les boutons icon-only hors un seul module déjà corrigé (PASS-5 C2, Critical).
- `sift-live.ts` grossit au lieu d'être splitté (942→1412 lignes) malgré une décision déjà actée (PASS-7 §2).
- Différenciateur produit réel noyé dans l'UI, jamais nommé "CDJ" dans le vocabulaire de l'écran le plus visité (PASS-6 Constat 1).
- Tests d'encodage qui passent silencieusement sans rien tester si les fixtures sont absentes (PASS-7 §3).

## Positionnement marché

**Le différenciateur unique de Sift est-il réel et défendable ?** Oui, sur deux points confirmés par le benchmark concurrentiel (PASS-9) : (1) la détection de faux lossless intégrée nativement dans un flux de gestion de bibliothèque — tous les concurrents audités (Rekordbox, Serato, Engine DJ, VirtualDJ) délèguent cette fonction à un outil tiers séparé ; (2) le ciblage hardware CDJ spécifique (32-bit float, header EXTENSIBLE, E-8305), absent de tout convertisseur généraliste (dBpoweramp/XLD/fre:ac) et de tout logiciel DJ audité. Seul Lexicon DJ atteint la parité sur le dédoublonnage par empreinte et l'intégration Discogs — Sift n'est donc pas seul sur le marché, mais reste unique sur la combinaison faux-lossless + CDJ dans un outil gratuit.

**Top 5 opportunités à fort levier** (PASS-9, classées effort/bénéfice, filtrées anti-bloat) :
1. Masquer/désactiver l'entrée nav "Rekordbox"/"Clé USB" simulée — effort Faible, urgence Critical.
2. Remonter "Compatibilité CDJ" dans le vocabulaire du verdict principal — effort Faible, ROI élevé (le seul argument vraiment unique face à tous les concurrents audités).
3. Étendre `report_json` au spectrogramme (fin du re-décodage systématique) — effort Faible, élimine un vrai goulot perf déjà prouvé.
4. Implémenter l'export Rekordbox XML réel via `rbox` — effort Élevé, mais ferme un vrai trou fonctionnel plutôt qu'un nice-to-have.
5. Import/migration depuis une bibliothèque Rekordbox/Serato existante — effort Élevé, réduit la friction d'adoption pour la cible la plus probable (DJ déjà équipé).

**Une ligne** : un DJ pro choisirait Sift plutôt que son outil actuel parce que c'est le seul outil gratuit qui détecte les faux lossless ET cible la compatibilité hardware CDJ nativement dans un flux de rangement automatique — à condition que l'export vers ce même hardware CDJ cesse d'être une promesse creuse.

## Top 20 des améliorations (classées par rapport effort/bénéfice)

1. **[Critical/Faible]** Masquer ou désactiver l'entrée nav "Rekordbox"/"Clé USB" tant qu'aucun backend n'existe — PASS-6 Constat 3, PASS-9 Opp.1.
2. **[Critical/Moyen]** Fermer le contournement du garde-fou anti-upscale (`rail_from_ext` doit consulter le contenu décodé, pas l'extension) — PASS-3 BUG-1.
3. **[Critical/Moyen]** Généraliser `aria-label` à tous les boutons icon-only (filing.ts, report-view.ts, journal.ts, ecartes-view.ts, library-detail.ts) — PASS-5 C2.
4. **[High/Faible]** Étendre `report_json` pour inclure le spectrogramme — élimine 100% des re-décodages — PASS-4 b2.
5. **[High/Faible]** Remonter "Compatibilité CDJ" dans le verdict principal de l'écran Revue — PASS-6 Constat 1, PASS-9 Opp.2.
6. **[High/Moyen]** Réparer `renderQueue` avec le pattern "create once, mutate" déjà présent dans `progress-zone.ts` — PASS-4 d1.
7. **[High/Faible]** Faire passer `ecartes::restore_track` par la primitive copy→verify→delete d'`actions::revert_one_fs` (bug cross-disk) — PASS-1 HIGH#1.
8. **[High/Moyen]** Fallback copy→verify→delete pour le filing conformant cross-disk (`std::fs::rename` échoue entre volumes) — PASS-1 HIGH#2.
9. **[High/Moyen]** Faire échouer bruyamment (pas de skip silencieux) les tests `encode.rs` sans fixtures en CI — PASS-7 §3.
10. **[High/Moyen]** Introduire un verrou applicatif par `track_id` entre le worker d'analyse et l'écriture de tags — PASS-3 BUG-3.
11. **[High/Moyen]** Ajouter un test qui force `commit_file` à échouer après un `execute_file` réussi pour exercer `rollback_fs` — PASS-7 §3.
12. **[Medium/Faible]** Traduire l'écran Bibliothèque en français et remplacer les styles inline par les classes `.sift-*` établies — PASS-5 H3.
13. **[Medium/Moyen]** Unifier les 3 patterns visuels de confirmation "action faite, annulable" en un seul système — PASS-5 H2.
14. **[Medium/Moyen]** Ajouter un jeton de séquence (`openSeq`) à `doIdentify`/`onIdentityApplied`, comme `openFilingInto` l'a déjà — PASS-5 H1.
15. **[Medium/Faible]** Réconcilier ou documenter explicitement l'écart entre le barème cutoff→kbps de `verdict.rs` et celui de `report-view.ts` — PASS-1 HIGH#3, PASS-2 §4, PASS-7 §5.
16. **[Medium/Faible]** Afficher immédiatement une barre 0/N au clic "Filer (n)" avant le premier événement `file:progress` — PASS-5 M1.
17. **[Medium/Faible]** Corriger le N+1 SQL de `list_filed` (une requête genres par piste) — PASS-4 d2.
18. **[Medium/Élevé]** Découper `sift-live.ts` (1412 lignes) en modules dédiés, sur le modèle de `chrome.ts`/`ecartes-view.ts` déjà extraits — PASS-7 §2.
19. **[Low/Faible]** Corriger l'espacement hors grille (`gap:7px`) dans `ecartes-view.ts` et le mauvais usage de la couleur "danger" pour un état non-erreur — PASS-5 M3/M4.
20. **[Low/Faible]** Retirer le paramètre `root` mort de la chaîne `trash_file_fs`/`trash_track` — PASS-1 LOW.

## Roadmap (ordre optimal d'implémentation)

**1. Corrections critiques (à faire avant toute autre chose)**
- Masquer l'entrée nav export simulée (#1).
- Fermer le contournement du garde-fou anti-upscale (#2).
- Généraliser `aria-label` (#3).

**2. Quick Wins (effort Faible, forte visibilité)**
- Étendre `report_json` au spectrogramme (#4).
- Remonter "Compatibilité CDJ" dans le verdict (#5).
- Fix cross-disk sur `restore_track` (#7).
- Barre de progression immédiate au clic "Filer" (#16).
- Corriger espacement/couleur `ecartes-view.ts` (#19).
- Retirer le paramètre `root` mort (#20).

**3. Améliorations importantes (effort Moyen, dette ou risque réel)**
- `renderQueue` create-once/mutate (#6).
- Fallback cross-disk sur le filing conformant (#8).
- Tests `encode.rs` bruyants sans fixtures (#9).
- Verrou applicatif worker/tagging (#10).
- Test de `rollback_fs` en échec (#11).
- Bibliothèque en français + classes `.sift-*` (#12).
- Unification des 3 patterns de confirmation (#13).
- Jeton de séquence sur l'identification Discogs (#14).
- Réconciliation du barème cutoff→kbps (#15).
- N+1 SQL genres (#17).

**4. Optimisations à long terme**
- Découpage de `sift-live.ts` (#18).
- Export Rekordbox XML réel via `rbox` (PASS-9 Opp.1, effort Élevé — condition pour retirer définitivement le masquage de l'étape 1).
- Import/migration depuis Rekordbox/Serato existant (PASS-9 Opp.3).
- Parallélisation du batch de filing (PASS-4 a2).
- Cache front du spectrogramme déjà calculé dans la session (PASS-4 c2, dépend de #4).

---

## Vérification finale — problèmes transversaux détectés en croisant les passes

Ces constats n'apparaissent pas dans une seule passe individuelle mais émergent en croisant plusieurs modules :

1. **Le double-miroir Rust↔TS n'est pas qu'un défaut de style, c'est un vecteur de régression silencieuse touchant directement la feature signature.** PASS-1 (architecture) le documente comme risque structurel, PASS-2 (qualité) prouve que les valeurs numériques divergent réellement, PASS-7 (maintenabilité) confirme la même duplication sous l'angle des tests manquants, et PASS-8 (vision) le classe comme la 3e incohérence la plus grave pour la confiance utilisateur. Quatre passes indépendantes convergent sur le même fichier (`report-view.ts:61-62` vs `analysis/verdict.rs:22-31`) — c'est un signal fort qu'il s'agit du point de dette le plus transversal du projet, plus qu'un problème isolé de qualité de code.

2. **Les deux défauts Critical (export simulé, garde-fou contournable) partagent une cause commune : l'écart entre ce que l'UI affirme et ce que le backend garantit réellement.** PASS-3 (bugs) prouve le contournement technique, PASS-6 (produit) prouve la simulation d'export, PASS-8 (vision) relie les deux comme "fonctionnalités qui mentent activement sur leur état" — le motif se répète : une garantie produit (rangement fiable, export fonctionnel) qui repose sur une supposition non vérifiée par le code (extension honnête, backend existant).

3. **Le god file `sift-live.ts` n'est pas seulement une dette de maintenabilité (PASS-7) — c'est aussi la cause probable directe du risque de course entre `doIdentify` et `doRanger` (PASS-5 H1) et de la violation `innerHTML=` en rafale (PASS-4 d1).** Les trois passes documentent des symptômes différents (couplage, race, perf) du même fichier surchargé sans jetons de séquence ni séparation état/rendu — un découpage (#18) réduirait la surface des trois problèmes simultanément, pas seulement la lisibilité.

4. **Le patron "best-effort silencieux" (avaler une erreur au lieu de la propager) apparaît à la fois en Rust et dans l'UX**, sous deux formes distinctes mais liées : côté Rust, `dedup.rs`/`filing.rs::load_tag_extras` confondent absence de donnée et erreur DB (PASS-2 §2) ; côté produit, l'absence de feedback immédiat au clic "Filer" (PASS-5 M1) ou au clic spectrogramme (PASS-4 b2, PASS-8) laisse l'utilisateur dans le doute sur ce qui se passe réellement. Le principe fail-fast documenté dans CLAUDE.md est bien respecté sur les crashs (0 `unwrap()` dangereux), mais moins sur la *visibilité* des dégradations silencieuses — un angle mort cohérent entre le code et l'UX qui n'aurait pas été visible en ne lisant qu'une seule passe.

5. **Aucune passe n'a trouvé de duplication de logique de mixage/lecture DJ-live** (attendu, hors scope Sift qui est un outil de préparation, pas de mix) — confirmation croisée que le périmètre produit reste cohérent avec le README malgré les défauts relevés : Sift ne dérive pas vers un scope qu'il ne devrait pas couvrir, le problème est l'exécution de son scope actuel, pas son étendue.
