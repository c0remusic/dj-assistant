# dir-01 — Hypothèses testables, par étage et par douleur

> [D1] = touche « l'archi tiendra-t-elle la suite ». [D2] = touche « pas pro / pas cohérent ».
> Chaque hypothèse est formulée pour être **prouvable ou réfutable** dans le code (phase 2).
> Recadrage : « la suite » = **M6b / M7 / 3 modes**, pas M2 (livré).

## Étage A — Stratégie produit

- **H-A1 [Produit]** « 8 features avant un seul utilisateur = dispersion ; Rekordbox natif /
  formatage clé sont des pièges de scope. » → tester : sont-elles **bâties** ou **gardées/différées** ?
- **H-A2 [Produit]** « Le geste reine est dilué : rien dans le produit ne le rend dominant face
  aux autres écrans. » → tester : hiérarchie réelle entre Revue et les 4 autres vues.
- **H-A3 [Produit]** « Les 3 modes de traitement (auto-règles / batch / détail) sont promis au
  plan mais le moteur de règles auto n'existe pas → risque de promesse non tenue / refonte. »

## Étage B — Architecture

- **H-B1 [D1]** « L'état UI éparpillé (DOM + vars de module dans 4 fichiers) ne tiendra pas
  l'état lourd de la suite → réécriture forcée. »
- **H-B2 [D1]** « Le contrat IPC tenu à la main (pas de codegen) → désync probable quand la suite
  ajoute des commandes. »
- **H-B3 [D1]** « Le mélange JS/TS (`allowJs:true, checkJs:false`) laisse `app.js` hors du
  type-check → des erreurs échappent. »
- **H-B4 [D1]** « Couplage front↔back (et front↔front via ids DOM partagés app.js↔sift-live)
  empêche d'itérer la suite sans casser l'existant. »
- **H-B5 [D1]** « `sift-live.ts` est un god-module qui grossit → maintenabilité en chute. »
- **H-B6 [D1]** « La `Mutex<Connection>` unique sera un goulot quand scan + workers + UI tapent
  ensemble sur 15 000 fichiers. »
- **H-B7 [D1]** « Les `#![allow(dead_code)]` masquent du code mort → dette invisible. »

## Étage C — Choix techniques

- **H-C1 [D1]** « Les décisions techniques (Symphonia, chromaprint, FFmpeg) sont documentées mais
  **pas réalisées** → dette latente entre le plan et le code. »
- **H-C2 [D1]** « Le profil de build / la stratégie de décodage ne tiendront pas le scan massif. »

## Étage D — UI / UX / design

- **H-D1 [D2]** « Pas de système de tokens → incohérence visuelle = sensation pas pro. »
- **H-D2 [D2]** « Même s'il existe des tokens couleur, la typo et l'espacement sont en dur
  partout → l'incohérence vient de là, pas des couleurs. »
- **H-D3 [D2]** « 7 vues de poids égal diluent le geste reine. »
- **H-D4 [D2]** « Le rendu inline multi-fichiers (app.js, sift-live, filing, report-view, library)
  fait diverger les écrans entre eux → incohérence inter-écrans. »
- **H-D5 [D2]** « Les messages d'erreur techniques bruts (`NO_TOKEN`, `NoLibraryRoot`) cassent le
  ressenti pro. »

## Méta

- **H-M1** « Les docs (README, CLAUDE.md) ont divergé du code → la compréhension de l'état (y
  compris la prémisse de cette mission) est faussée. »
