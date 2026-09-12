# Audit final du chantier d'évolution architecturale

Date : 2026-08-05. Audit demandé explicitement par Antoine « une fois tout livré », les
5 phases l'étant.

**Méthode.** L'audit se fait **contre le diagnostic d'origine**, ligne par ligne. La spec
n'est plus sur le disque (sortie du suivi par `6551728`, passage de `docs/` en liste
blanche) mais reste récupérable :

```bash
git show 6551728^:docs/superpowers/specs/2026-07-13-architecture-evolution-design.md
```

Chaque verdict ci-dessous est rattaché à une preuve re-dérivée du code **actuel**, pas
reprise d'un rapport de phase. Trois catégories : **traité**, **écarté avec motif mesuré**,
**encore ouvert**. « Écarté » n'est pas « traité » : c'est une décision de ne pas faire,
appuyée sur une mesure.

## Verdict par ligne du diagnostic

| # | Affirmation d'origine | Verdict | Preuve re-dérivée |
|---|---|---|---|
| 1 | `sift-live.ts` orchestrateur très large (2083 lignes) | **Traité** | `wc -l frontend/sift-live.ts` = **570** (−73 %). Extractions : `rekordbox-view.ts`, `queue-panel.ts`, `batch-panel.ts`. |
| 2 | Contrats IPC mirrorés à la main, aucun test round-trip | **Traité** | **14** tests `*_matches_contracts_ts` répartis sur 8 modules (`analysis/mod.rs`, `dedup.rs`, `filing.rs`, `ipc_filing.rs`, `library.rs`, `naming.rs`). Destructuration exhaustive : un champ Rust ajouté casse la compilation. |
| 3 | `list_library`/`list_queue` renvoient toutes les lignes | **Écarté, motif mesuré** | Toujours sans `LIMIT` (`grep LIMIT` sur `library.rs` + `queue.rs` : vide). Phase 3 : 15k lignes fluides, 100k à 165–250 ms ; l'index `metadata(artist,title)` a été mesuré **après ajout**, sans gain, et retiré avant commit. Déclencheur de réouverture nommé (bibliothèque réelle > 30k `filed` **et** lenteur perçue). |
| 4 | Dédup avec chemin O(n²) | **Traité** | Migration **v19** : `dup_edges` + `dup_scanned` (`db.rs:316-330`), l'invariant étant que toute paire de `dup_scanned` a été évaluée. `group_duplicates` et `load_dup_scan_rows` sont passés `#[cfg(test)]` — le compilateur refuse désormais tout appelant de production. |
| 5 | `report_json` garde rapport **et** spectrogramme | **Traité — mais un commentaire ment** | `worker_loop` passe `false` pour la collecte de grille (`worker.rs:325`) : la grille n'est plus conservée, elle se recalcule à l'ouverture du collapse. Base 4,11 Go → 119 Mo. **Voir F1.** |
| 6 | Pool d'analyse et encodeurs FFmpeg sans budget partagé | **Partiellement traité, et JAMAIS mesuré** | **Voir F2.** C'est la seule ligne qui n'est ni traitée, ni écartée sur mesure. |
| 7 | Connexion SQLite partagée → contention malgré WAL | **Écarté, motif mesuré** | `bench_sqlite.rs::bench_sqlite_lock_wait_under_analysis_load`. 0 `SQLITE_BUSY`, verrou attendu 200 ns p50, tenu 0,4 % du temps. Les cinq changements d'architecture envisagés par la spec sont rejetés par cette mesure. |

Contrôle transverse : `db.rs::MIGRATIONS` compte **20** entrées, et l'app en cours
d'exécution rapporte `db schema=20` (ligne `SMOKE OK` du log `tauri dev`, relevée le
2026-08-05). Le compte statique et l'état vivant concordent.

## F1 — Un commentaire périmé décrit l'inverse du code

`src-tauri/src/worker.rs:112` dit encore, au-dessus du paramètre `report_json` :

> `cache the full report, spectrogram included (FIX-3) — instant re-open AND instant spectrogram, no re-decode either way.`

C'est faux depuis le 2026-08-03, et c'est **contredit par le code 210 lignes plus bas** :
`worker.rs:325` passe explicitement `false` pour la collecte de la grille, avec un
commentaire de 18 lignes qui chiffre le marché refusé (~450 ko par piste contre 631 ms
gagnées à l'ouverture du collapse).

Gravité : pas de bug d'exécution, mais c'est le seul endroit qu'un lecteur consulte pour
savoir ce que `report_json` contient, et il répond l'inverse de la vérité. Le prochain qui
enquête sur la taille de la base part dans la mauvaise direction.

**Correctif** : réécrire le commentaire de `:112` pour dire ce que le champ porte
réellement (verdict + waveform, pas la grille) et renvoyer au commentaire de `:325` qui
porte la décision et ses chiffres.

## F2 — La ligne 6 est déclarée close sans jamais avoir été mesurée

C'est la trouvaille principale de cet audit.

**Ce que le diagnostic affirmait** : analyse et encodage calculent leur parallélisme
indépendamment, sans budget commun.

**Ce qui est vrai aujourd'hui.** Les deux pools sont toujours calculés séparément :

- analyse — `available_parallelism().clamp(1, 8)` (`worker.rs:156-159`), jusqu'à 8 threads ;
- encodage — `(cores / 2).max(1).min(4)` (`ipc_filing.rs:639-642`), sous-souscription
  délibérée parce que chaque process FFmpeg est lui-même multi-thread.

**Ce qui s'est réellement amélioré** : l'encodage a gagné un **plafond global** partagé
entre le chemin interactif (`run_file_track`) et le pool de phase 2 du batch
(`ipc_filing.rs:645-650`). Avant, `file_track` ne se bornait que par piste
(`ALREADY_FILING`) et N clics sur N pistes lançaient N FFmpeg. C'est un vrai correctif,
mais il borne l'encodage **avec lui-même**, pas avec l'analyse.

**Ce qui n'existe pas** : aucune mesure de contention CPU dans tout l'arbre.
`grep -rn "oversubscri\|contention CPU\|cpu_budget"` sur `src-tauri/src/` ne rend que deux
**doc-comments qui raisonnent** sur la sur-souscription (`ipc_filing.rs:637` et `:650`) —
aucun benchmark, aucun chiffre.

**Pourquoi la Phase 5 ne la couvre pas.** Ses mesures vivent dans `bench_sqlite.rs` et
portent sur l'attente du **verrou SQLite** sous charge d'analyse. C'est une autre question :
un `Mutex<Connection>` tenu 0,4 % du temps ne dit rien de 8 threads de FFT tournant en même
temps que 4 process FFmpeg multi-thread sur la même machine. Compter la ligne 6 comme
« rejetée par la Phase 5 » serait un report de conclusion d'une question sur une autre.

**Verdict au moment du constat** : ni traitée, ni écartée. C'était la seule des sept.

### La mesure a été faite, le même jour

`src-tauri/src/bench_cpu_budget.rs`, ajouté pour cet audit. Il n'y avait pas de raison de
laisser la ligne ouverte une fois le manque identifié.

**Forme de la mesure.** Pas trois chronos sur un même lot — cette première version a été
écrite, exécutée, et **jetée sur son propre résultat** : l'analyse mettait 1,49 s là où
l'encodage mettait 28,13 s, donc « les deux ensemble » valait mécaniquement « encodage
seul », et l'écart résiduel ne mesurait que le cache disque réchauffé entre les passages —
au point de rendre une position *négative*, sous la borne de cohabitation parfaite. Elle
avalait aussi ses erreurs (`let _ = analyze(...)`), si bien qu'une phase entièrement en
échec se serait lue comme une phase rapide.

La version retenue compare des **débits** sur des fenêtres de 20 s : chaque charge seule,
puis pendant que l'autre tourne, avec préchauffage hors mesure et comptage explicite des
succès et des échecs. Les tailles de pool ne sont pas recopiées : `worker::analysis_pool_size`
et `ipc_filing::phase2_worker_count` sont appelées telles quelles, ce qui a demandé de les
rendre `pub(crate)` — une copie aurait dérivé, exactement le défaut que cet audit reproche.

**Résultat** (16 cœurs, pool analyse 8, pool encodage 4, lot de 24 fichiers réels) :

| charge | seule | pendant l'autre | débit conservé |
|---|---|---|---|
| analyse | 9,18 fichiers/s (193 ok, 0 échec) | 8,25 fichiers/s | **89,9 %** |
| encodage | 0,40 fichiers/s (11 ok, 1 échec) | 0,24 fichiers/s | **59,9 %** |

Somme : **149,8 %**, où 200 % signifierait cohabitation parfaite et 100 % ressource saturée.

**Ce que ça règle.** La contention existe, elle est réelle et mesurée — l'affirmation
« aucun budget partagé » du diagnostic décrivait bien quelque chose. Mais ce n'est pas un
effondrement : rien ne se sérialise.

**Ce que ça règle surtout, et qui n'était pas prévu : elle est asymétrique.** L'analyse ne
perd que 10 % de son débit, l'encodage en perd 40 %. C'est cohérent avec la configuration —
8 threads de FFT face à 4 process FFmpeg eux-mêmes multi-threads sur 16 cœurs : c'est
l'encodeur qui absorbe la sur-souscription. Traduit en gêne utilisateur : **convertir des
pistes pendant qu'une analyse tourne prend environ 1,7× plus longtemps**.

**Ce que ça NE règle pas.** Que ce facteur 1,7 soit acceptable ou non n'est pas une question
de mesure. Un sémaphore partagé le supprimerait en ralentissant l'analyse — qui, elle, est
le chemin par lequel l'utilisateur attend son verdict. C'est un arbitrage produit, à trancher
par Antoine, et il est désormais posé sur des chiffres au lieu d'une intuition.

**Verdict final de la ligne 6** : **mesurée**. Plus « comptée close sans preuve », plus
« ouverte faute de mesure » — un arbitrage explicite en attente, ce qui est la seule position
défendable des trois.

## Ce que cet audit ne couvre pas

- **La qualité interne des modules extraits en Phase 1.** L'audit vérifie que
  `sift-live.ts` a maigri de 73 %, pas que les trois modules qui l'ont absorbé sont bien
  découpés.
- **Le comportement runtime.** Tous les verdicts sont statiques, sauf le contrôle
  `schema=20` relevé sur l'app en marche. Aucun scénario utilisateur n'a été rejoué.
- **Les travaux d'une session concurrente.** Le 2026-08-05 à 10:34–10:37, une autre session
  ajoutait vitest + eslint à ce dépôt (`package.json`, `tsconfig.json`, `test/`,
  `eslint.config.js`, `.github/workflows/test.yml`, `CLAUDE.md`). Aucun recoupement avec les
  7 lignes du diagnostic, mais le `CLAUDE.md` cité ici peut avoir bougé depuis.

## Conclusion

Six lignes sur sept sont fermées et défendables : **4 traitées** (1, 2, 4, 5), **2 écartées
sur mesure** (3, 7). Les deux « écartées » valent autant que les traitées — dans les deux
cas la mesure a précédé la décision, et dans le cas de la ligne 3 elle a fait **retirer**
un index déjà écrit.

La ligne 6 a été mesurée dans la foulée de son constat, le même jour : contention réelle et
**asymétrique** — 90 % de débit conservé côté analyse, 60 % côté encodage, soit un facteur
~1,7 sur une conversion lancée pendant une analyse. Son défaut n'aura donc pas été d'être
ouverte, mais d'avoir été comptée close sans preuve.

Ce qui reste n'est plus une mesure manquante mais un **arbitrage produit** : ce facteur 1,7
est-il acceptable, sachant qu'un sémaphore partagé le supprimerait en ralentissant le chemin
par lequel l'utilisateur attend son verdict ? Personne ne peut trancher ça avec un
benchmark.

Plus deux corrections de documentation : **F1 appliquée** (`worker.rs:112` réécrit), et le
compte de `sift-live.ts` que la mémoire de projet donnait à 520 lignes alors qu'il en fait
570.
