# RAPPORT — Verdict de direction (Sift)

> Audit détective-architecte, **lecture seule**, 2026-06-25, branche `m6a-discogs`.
> Méthode : théorie → preuve → proposition chiffrée. Détail : `dir-00`…`dir-03`.
> Aucun fichier de code modifié. À lire ensemble ; tu décides ensuite.

## 0. Le recadrage qui change tout

La prémisse de la mission (« M0+M1 faits, **M2 = prochaine grosse pièce** ») est **fausse** —
elle vient du **README périmé**. Réalité prouvée : **M0→M6a livrés, M6b en cours** (analyseur,
player, dédup, filing, Discogs : tout tourne). Donc :
- La question « l'archi tiendra-t-elle pour M2 ? » est **caduque** : M2 (la pièce la plus lourde
  en état UI : lecture/waveform/verdict) **est déjà passée sans réécriture**.
- La vraie question = « tiendra-t-elle pour **M6b / M7 / le mode auto-règles** ? ».

## 1. Niveau de remise en question que les PREUVES justifient : **CORRECTIONS CIBLÉES**

Pas une refonte, pas même une refonte partielle d'un étage. Justification :
- **Étage A (produit)** : RÉFUTÉ que ce soit dispersé — scope **séquencé et gardé** (Rekordbox/clé
  différés + masqués). Sain.
- **Étage C (technique)** : RÉFUTÉ que les décisions soient en l'air — **toutes réalisées**
  (Symphonia câblé, hybride FFmpeg, chromaprint). Sain.
- **Étage B (archi)** : les risques PROUVÉS (god-module, contrat DOM implicite, connexion unique,
  dead_code) sont **connus, locaux et bornés** — déjà catalogués par 2 audits. Aucun n'impose une
  réécriture ; ce sont des **corrections ciblées**.
- **Étage D (UI)** : RÉFUTÉ qu'il n'y ait « pas de système » — les **couleurs sont tokenisées**.
  Le défaut PROUVÉ est précis et **local** : pas d'échelle typo/espacement.

→ La direction est **bonne**. Ce qu'il faut, ce sont **quelques corrections nettes**, pas un
changement de cap. Le plus gros risque réel n'est pas l'archi : c'est la **divergence doc↔code**
qui fausse le pilotage (cette mission en est la preuve vivante).

## 2. Les 3 actions à plus fort levier AVANT de continuer (M6b/M7)

Prouvées, chiffrées, classées par ROI :

1. **P-1 — Échelle typo + espacement tokenisée** (D2). *Preuve : 144 font-size inline, 0 échelle.*
   Coût : **0,5 j** + migration ~0,25 j/écran, risque faible. → **règle direct la douleur « pas
   pro »**, et tu arrêtes de deviner des tailles au cas par cas.
2. **P-2 — Réconcilier README + CLAUDE.md** (D1 indirect). *Preuve : README « M2 à venir ».*
   Coût : **1–2 h**, risque nul. → supprime la source des mauvaises décisions de cap.
3. **P-3 — Split SÛR de `sift-live.ts`** (1144 l → 3 extractions déjà nommées) (D1). *Preuve :
   doublé depuis le dernier audit.* Coût : **0,5–1 j**, risque moyen, **avant** que M6b le regrossisse.

Cheap add-ons quand tu y es : **P-4** (assertions DOM, 2–3 h, fail-fast) et **P-5** (retirer les
`#![allow(dead_code)]`, 2–4 h). Total des 3 majeures ≈ **2–3 j-IA**.

## 3. Ce qui est SAIN — à préserver (ne pas casser en améliorant)

- **Sécurité fichiers** : tout passe par le journal `actions` + corbeille réversible ; undo partout.
- **Invariant deux rails / jamais d'upscale** (lossy↛lossless) — cœur produit, bien gardé.
- **Hybride Symphonia (analyse) + FFmpeg (encode)** — mesuré, justifié, réalisé.
- **Discipline async front** : `openSeq`/`currentWs`/invalidation de cache (report-view) — c'est ce
  qui fait que l'état éparpillé tient. Ne pas le défaire en « simplifiant ».
- **Tokens couleur** (198 usages) + **fonts self-hosted** (offline).
- **Couverture de tests** : 150 tests lib, clippy `-D warnings`, tsc — et **l'habitude d'auditer**.
- **Le binding honnête UI↔fonctions réelles** (pas de données fabriquées) — discipline rare, à tenir.

## 4. Arbitrages qui te reviennent (produit / goût, pas technique)

1. **Mode « auto par règles »** (H-A3) : il est annoncé *par défaut* mais **n'existe pas**. Le
   bâtir avant M7, ou assumer une V1 « revue + batch » et repousser ? **Décision produit.**
2. **M5 : AcoustID en ligne ou dédup local seulement ?** Tranche le sort de `chromaprint-next`
   (migrer seulement si online). **Décision produit** (ça débloque ressources-externes).
3. **Avenir de la couche `app.js`** : la garder comme maquette navigateur (et vivre avec le contrat
   DOM implicite), ou la collapser en un seul rendu TS (élimine 3 problèmes d'un coup, mais ~2–3 j) ?
   **Décision de goût/temps** — pas urgent, mais à trancher avant que le front grossisse encore.
4. **Hiérarchie visuelle du geste reine** (H-A2/D3, faible) : veux-tu que « Revue » domine
   visuellement les autres vues, ou le badge suffit ? **Goût.**

## 5. Laissé INDÉTERMINÉ (faute de preuve) — et ce qu'il faut pour trancher

- **Magnitude réelle du goulot `Mutex<Connection>`** (H-B6) à l'échelle 15 000 fichiers : **non
  mesurée**. Pour trancher P-6 : un **test de charge** scan+analyse sur un gros dossier réel +
  compter les `SQLITE_BUSY` déjà loggués. **Tant que non mesuré → ne pas faire le pool** (gros
  coût, gain non prouvé).
- **Existence de casses silencieuses du contrat DOM** (H-B4) : indéterminé aujourd'hui ; les
  assertions de P-4 les **feraient apparaître** (c'est leur but).
- **Désync IPC réelle** (H-B2) : aucune constatée, mais non instrumentée. Un seul **test de
  round-trip** par struct partagée suffirait à fermer la question sans codegen.

---

### En une phrase
**La direction de Sift est saine et plus avancée que ne le disent ses propres docs ; ne refonds
rien — fais 2–3 jours de corrections ciblées (échelle typo, docs, split sift-live), mesure le seul
risque archi sérieux (DB) avant de le « réparer », et garde tes invariants.**
