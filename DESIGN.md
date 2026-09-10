# DESIGN.md — socle de design Sift

> Créé le 2026-08-19, phase 2 de la reprise UI/UX (skill `sift-macos-ui`).
> Ce fichier est la **couche de décision**. Il ne remplace pas `frontend/styles.css`,
> qui reste la **valeur** canonique des tokens. Un chiffre écrit ici sans être dans
> `styles.css` est une proposition, pas un fait.

## Précédence

Quand deux sources se contredisent, l'ordre est :

1. **Ce fichier**, pour les décisions systémiques validées.
2. **`frontend/styles.css`**, pour la valeur exacte d'un token.
3. **`docs/design-system/`** (6 fichiers) et `docs/design-system-states.md`, pour
   l'historique daté et les états réels des composants.
4. **Apple HIG**, pour toute question que les trois précédents ne tranchent pas.

Une recommandation qui ne s'appuie sur aucun des quatre se marque
« proposition, sans source ».

## Ce que ce socle ne décide pas

Les specs d'écran sont la **phase 4**, dans `docs/ui-specs/<vue>.md`. Ce fichier n'en
tient que l'index. Le shell et le mapping des écrans, eux, sont ici — sections 14 à 17.

---

## 1. Produit et utilisateur

Sift est un poste de décision avant le set, jamais pendant le live. Principe
central : **déplacer = encoder + ranger**.

L'utilisateur cible est un DJ qui traite de gros volumes — la bibliothèque est
dimensionnée à 15k–100k lignes dans les bancs du dépôt (`src-tauri/src/bench_volume.rs`).
Il accepte la densité si elle est lisible, stable et honnête. Il vit au clavier.

Conséquences non négociables :

- densité avant décoration ;
- vocabulaire métier, libellés courts, aucun ton pédagogique ;
- aucune information critique portée par la seule couleur ;
- un état permanent reste neutre ; seule sa transition se colore, brièvement.

Barre de qualité : le niveau **et la structure** des apps macOS système. Chaque vue
reprend le patron de celle qui a déjà résolu son problème — Finder et Music pour
parcourir, Réglages Système pour configurer, Utilitaire de disque pour une opération
longue, Moniteur d'activité pour du temps réel.

Anti-références : dashboard SaaS décoratif, landing page, suite audio surchargée,
interface consumer colorée, empilement de cartes sans hiérarchie.

---

## 2. Typographie

### Familles

| Rôle | Token | Valeur |
|---|---|---|
| Interface | `--font-ui` | `"Inter", -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif` |
| Nombres, chemins, mesures | `--font-mono` | `"JetBrains Mono", ui-monospace, SFMono-Regular, Menlo, monospace` |

Les deux familles sont auto-hébergées via `@fontsource` (`frontend/main.ts`) : l'app
ne fait aucune requête réseau pour une police. **Trois graisses chargées par famille**
— 400, 500, 600. Ne jamais écrire une graisse non chargée : le moteur ne signale
rien, il synthétise, et `document.fonts.check()` répond `true` même sans la face.
`test/font-weights.test.ts` confronte les imports de `main.ts` aux graisses demandées
par `styles.css` à chaque `npm run test`.

### Échelle

Sept crans, **seules valeurs autorisées** :

```
--text-xs:10px  --text-sm:11px  --text-md:12px  --text-base:13px
--text-lg:15px  --text-xl:20px  --text-2xl:32px
```

Deux registres, et la coupure entre eux est le point de l'échelle :

- **Donnée — 10 à 13 px.** Listes, tables, métadonnées, chips. Environ 80 % du texte
  peint. Ces quatre crans ne bougent pas : ce sont eux qui portent la densité.
- **Titre — 15, 20, 32 px.** L'écart avec le registre donnée est ce qui fait lire un
  titre comme un titre.

`--text-base:13px` est exactement la taille par défaut de macOS dans les HIG.
`--text-xs:10px` est exactement le plancher de lisibilité qu'elles fixent. **Rien ne
descend sous 10 px**, ni par token ni par littéral. Deux crans inférieurs ont existé
(9 px, 8 px) et ont été retirés le 2026-08-05.

**Discipline par écran (ajout du 2026-08-21).** L'échelle autorise sept crans dans
l'app ; un **écran** n'en emploie que **trois ou quatre** — un titre, un corps, une
légende — sous peine de bruit. La hiérarchie se fait d'abord par l'**encre** (§ 4 :
primary / secondary / tertiary), pas par une pluie de tailles proches. Le **monospace**
(`--font-mono`) est réservé aux **chiffres qui s'alignent en colonne** (durées, BPM,
mesures) avec `tabular-nums` ; il ne sert **pas** à « faire technique » sur un chemin, un
format ou un temps isolé — ceux-là restent en `--font-ui`. Les **chemins de fichier** se
rendent en `--font-ui` et en **segments** (`Dossier › fichier`, patron *path control* des
HIG), jamais en chaîne monospace tronquée. Mesure du 2026-08-21 : 25 usages de
`--font-mono` dans `styles.css`, source du « fouillis » relevé sur la surface Revue.

### Tracking

```
--tracking-normal:.01em  --tracking-wide:.03em
--tracking-wider:.05em   --tracking-widest:.06em
```

Le tracking se dérive de la **taille**, jamais du goût : plus la taille descend, plus
le tracking s'ouvre. Les en-têtes de colonne en capitales (`.col-h`) emploient
`--tracking-wider`.

---

## 3. Grille et espacement

Base 4. Six valeurs, **seules autorisées** :

```
--space-4  --space-8  --space-12  --space-16  --space-24  --space-32
```

Règles d'application :

- **4** — écart intra-composant (icône ↔ libellé, deux lignes d'un même bloc).
- **8** — écart entre éléments frères d'un même groupe.
- **12** — écart entre groupes dans un panneau. C'est le pas par défaut de
  `.sift-screen-stack`.
- **16** — padding intérieur d'une boîte, écart entre deux panneaux.
- **24** — marge de fenêtre. `#content` l'emploie sur ses quatre côtés.
- **32** — respiration exceptionnelle, à justifier.

`npm run lint:tokens` refuse un espacement en dur qui contourne un token.

### Rayons

**Re-racinés sur le kit macOS Big Sur le 2026-08-25** (commit `b973ce3`). Un seul nombre ne
pilote plus l'échelle : les crans de contrôle sont des **valeurs relevées composant par
composant** sur le kit (pont Figma REST, fileKey `k3ek2XpmIKjqiFUsyn5kCi`), chacune avec le
nœud qui la porte.

```
--border-radius-xs:4px    /* case à cocher — Buttons/Checkbox 44:314 */
--border-radius-sm:5px    /* item de menu, champ texte, segmenté — Menu-items 58:49,
                             Text-fields 45:1008, Segmented 47:23 */
--border-radius-md:7px    /* bouton, champ de recherche, pulldown, bouton icône —
                             Push 43:43, Search-fields 45:812, Pulldown 45:152, Icon 45:88 */
--border-radius-base:14px
--border-radius-lg:var(--border-radius-base)   /* grandes surfaces — inchangé */
--border-radius-pill:999px
```

**Pourquoi la dérivation est tombée.** L'échelle précédente partait de 14 avec
`sm = base − 6` (8 px) et `md = base − 4` (10 px) : aucun cran ne tombait sur une valeur du
kit, donc les neuf composants livrés de Revue étaient tous décalés d'un cran vers le rond —
pour une seule et même raison. Apple donne **une valeur par composant**, pas une échelle ; le
`calc()` était la mauvaise forme pour cette source. Antoine a tranché le re-racinage plutôt
que l'ajout de crans, tous les écrans devant changer de toute façon : les ~107 usages de
l'app bougent avec, et c'est l'intention, pas un effet de bord.

`base` / `lg` (14) **ne vient pas du kit** et ne bouge pas ; les grandes surfaces seront
confrontées au kit quand sheets et alertes y seront mesurées.

Rayon imbriqué : quand le kit **ne donne pas** de valeur pour la surface en cause, il se
dérive du conteneur, jamais choisi séparément — un élément dans une boîte `lg` prend `md` ;
dans une boîte `md`, il prend `sm`. Quand le kit **donne** la valeur du composant, c'est elle
qui gagne sur la dérivation.

### Mesures et hauteurs

| Token | Valeur | Rôle |
|---|---|---|
| `--measure-data` | 1200px | Largeur maximale d'une surface portant de la **donnée mesurée** (spectrogramme). ⚠️ Dupliqué côté Rust dans `analysis::spectrum::MAX_COLS` et épinglé par `analysis::spectrum::tests::css_data_measure_matches_max_cols` — éditer l'un sans l'autre fait tomber `cargo test`. |
| `--h-40` | 40px | Hauteur de contrôle. Seule hauteur déclarée : le dépôt supprime tout token sans consommateur. |
| `--titlebar-h` | 30px | Barre de titre custom. Partagée entre `chrome.ts` et le bandeau de mise à jour. |

**Règle du dépôt, à respecter dans ce fichier aussi : on ne déclare pas un token qui
n'a pas de consommateur.** `--h-32` et `--h-44` ont été supprimés pour cette raison.

---

## 4. Couleur

Toutes les valeurs sont en **OKLCH**, dans `frontend/styles.css`. Ce fichier n'en
recopie aucune : la valeur exacte se lit là-bas.

### Trois blocs, toujours les trois

Une édition de token doit rester cohérente dans :

1. `:root` (`styles.css:11`) — thème clair ;
2. `@media (prefers-color-scheme:dark) > :root:not([data-theme="light"])` (`:266`) ;
3. `:root[data-theme="dark"]` (`:329`) — override explicite du réglage Apparence.

Comparer les valeurs **résolues** des deux tokens dans les deux thèmes, pas leurs noms.
⚠️ Une feuille injectée pour tester un override doit s'écrire `:root:root:root` : le
bloc sombre du dépôt est de spécificité (0,2,0), un `:root` simple perd silencieusement.

### Neutres — quatre plans, pas plus

```
fond  <  chrome  <  contenu  <  surélevé
```

`--color-background-primary` (fond) · `--color-background-tertiary` (chrome : rail de
navigation et colonne de file, même couche) · `--color-background-secondary` (contenu,
et **aussi** l'état actif : un élément sélectionné est promu au niveau du contenu, il
n'a pas de gris à lui) · `--color-surface-raised` (popover, bouton surélevé).

Base neutre = gris système d'Apple, teinte H≈286. Provenance citable : table
« iOS system gray colors » des HIG, valeurs publiées en texte. Écart assumé et nommé :
ce sont les gris **iOS**, Apple ne publiant aucune valeur chiffrée pour macOS.

### Sémantique — quatre teintes, un sens chacune

| Teinte | Sens | Ne jamais employer pour |
|---|---|---|
| Vert (`success`) | Sain, lossless authentique, terminé | Une progression de lecture |
| Ambre (`warning`) | Doute, décision attendue, en attente | Un risque réel |
| Rouge (`danger`) | Risque réel, faux lossless, destructif | Un simple avertissement |
| Bleu (`info`) | Interactif, focus, sélection, lien | Un verdict |

Quatre niveaux d'encre : `--color-text-primary` → `quaternary`. Les fonds sémantiques
sont **opaques**, jamais en alpha : un fond alpha se compose différemment selon le gris
sous-jacent, et le contraste mesuré descendait à 3,37:1 sur les cartes les plus sombres.

### Aplat d'accent — le fond d'un bouton primaire, et rien d'autre

`--color-accent-fill` + `--color-accent-ink`, ajoutés le 2026-08-19. Un bouton primaire macOS
est un **aplat** d'accent avec du texte blanc (kit Big Sur, § 02-Buttons / 01-Push Buttons) ;
Sift n'avait aucun token pour ça — `--color-background-info` est un fond *pâle* et
`--color-hue-blue-solid` interdit le texte par-dessus, son propre commentaire le dit.

Les deux sont **theme-invariants**, comme `--color-text-on-scrim` : ce couple ne se pose pas sur
une surface de l'app, il **est** sa propre surface, et c'est le rapport interne de la paire qu'on
veut constant.

La valeur **est** le systemBlue d'Apple (`--color-hue-blue-solid`, L 60,28 %). Blanc dessus mesure
~3,9:1, donc sous le plancher AA — voir la portée ci-dessous.

⚠️ Le **fond** d'un bouton secondaire ne fait que 1,86:1 contre la page, sous le 3:1 des HIG pour un
élément d'interface non textuel. Le kit fait le même écart (~2,3:1 sur carte sombre) : la lisibilité
d'un bouton vient de son texte, mesuré à 8,12:1.

### Portée du plancher de contraste — révisée le 2026-08-19

Décision d'Antoine, et elle prime sur ce que la section dit plus haut : **Sift est son outil
personnel, pas un produit à auditer.** Le plancher AA 4,5:1 cesse d'être une contrainte bloquante.

Ce que ça change : un arbitrage entre *fidélité au kit* et *ratio de contraste* se tranche
désormais en faveur de la fidélité. `--color-accent-fill` en est le premier cas — il avait été
assombri à L 46 % pour atteindre 7:1, il est rendu à la valeur d'Apple.

Ce que ça ne change pas : la **lisibilité** reste un critère de qualité, pour un lecteur d'un seul
utilisateur comme pour mille. Un texte qu'on ne lit pas confortablement reste un défaut, mesuré ou
non. Et les deux règles issues de défauts réels tiennent, parce qu'elles ne parlent pas
d'accessibilité mais d'honnêteté : l'atténuation d'un état ne se fait pas à l'opacité, et « échec »
est l'information qu'on n'a pas le droit d'estomper.

Corollaire pratique : aucun attribut ARIA n'est ajouté aux composants neufs. Ceux déjà en place
restent — les retirer coûterait du temps pour rien et casserait des sélecteurs de test.

Deux encres qui ne basculent **jamais** avec le thème, parce qu'elles se posent sur des
pixels d'image et non sur une surface de l'app : `--color-text-on-scrim` et les deux
scrims `--overlay-scrim` / `--overlay-scrim-caption`.

### Accents catégoriels

Les neuf `--color-hue-*-solid` sont réservés aux **taxonomies** : familles de genre,
segments de graphique d'occupation, couleur de dossier source. Jamais un titre de
section, jamais un état. Les variantes `-bg` / `-text` sont des couleurs de puce ; les
`-solid` sont des aplats de donnée et ne portent jamais de texte.

### Contraste

Plancher **AA 4,5:1** partout. Cible **7:1** pour une couleur personnalisée sur du
petit texte, conformément aux HIG Dark Mode.

Deux règles issues de défauts réels :

- **L'atténuation d'un état ne se fait pas à l'opacité.** Aucune valeur d'opacité ne
  franchit 4,5:1 avant ~0,92, où l'atténuation ne se voit plus. Le levier est le token
  (`--color-text-secondary` au lieu de `primary`).
- **« Échec » est l'information qu'on n'a pas le droit d'estomper.** Un fichier dont
  l'analyse a échoué doit se voir **mieux** que les autres, pas moins bien.

---

## 5. Surfaces — la grammaire de boîte

### État mesuré le 2026-08-19

Cinq classes se disputent le même travail. Comptage exact des occurrences dans
`frontend/` et `index.html` :

| Classe | Occurrences | Ce qu'elle fait vraiment |
|---|---|---|
| `.sift-ui-card-soft` | 19 | Fond secondaire + bordure |
| `.sift-ui-card-soft-pad` | 14 | Son padding (16 px) |
| `.sift-ui-card-outline` | 8 | Bordure seule — **7 usages sur 7 dans `rekordbox-view.ts`**, chacun avec `padding:10px 12px` en style inline |
| `.sift-ui-card` | 7 | Fond, sans bordure |
| `.sift-ui-card-pad` | 4 | Son padding |

`.sift-ui-card-outline` n'est pas un troisième niveau sémantique : c'est **la ligne de
Rekordbox**, rembourrée à la main sept fois, avec un `10px` qui n'appartient à aucune
échelle.

### Décision

**Deux niveaux, pas cinq.**

1. **Contenu** — `.sift-ui-card` : fond, pas de bordure. La surface sur laquelle vit la
   donnée.
2. **Chrome doux** — `.sift-ui-card-soft` : fond secondaire + bordure. Un groupe de
   contrôles, un panneau latéral, un avertissement.

Le padding reste une classe séparée (`-pad`, `-soft-pad`) pour qu'une boîte sans
padding reste possible.

`.sift-ui-card-outline` disparaît : ses 7 sites deviennent une classe de **ligne**
propre à Rekordbox, avec un padding pris dans l'échelle. Le geste appartient à la
phase 3, pas ici.

**Une boîte est une décision, pas une décoration.** Si un bloc ne contient qu'une seule
information, il ne prend pas de boîte : l'espacement suffit à le grouper.

---

## 6. Motion

```
--duration-fast:75ms   --duration-base:150ms   --duration-slow:300ms   --ease-out:cubic-bezier(.2,0,0,1)
```

### État mesuré le 2026-08-19

`frontend/styles.css` porte **37 déclarations `transition:`**, dont **2** référencent un
token de durée. Les littéraux emploient **9 durées distinctes** : `.08s .1s .12s .15s
.16s .18s .2s .25s .3s`. Une échelle à deux valeurs, neuf valeurs en usage.

### Décision

Trois durées, et elles couvrent tout. ⚠️ Les valeurs ci-dessous sont celles de la
**racine tranchée par Antoine sur l'issue #10** (2026-08-19, échelle 75/150/300, référence
150 = médiane du cluster portant 30 des 44 déclarations d'alors). Ce document avait posé
100/250 le même jour sans avoir lu le ticket ; recalé le 2026-08-20, `styles.css` fait foi.

| Token | Valeur | Emploi |
|---|---|---|
| `--duration-fast` | 75 ms | Micro-retour sous le doigt : survol, focus, pression, bascule |
| `--duration-base` | 150 ms | **Référence** — tout retour d'action : sélection, ouverture de popover, apparition de chip |
| `--duration-slow` | 300 ms | Ample — déplacement de matière : pouce de contrôle segmenté, panneau qui s'ouvre, progression |

Par-dessus l'échelle, la règle d'Apple (`/motion`) que #10 retient : **zéro mouvement sur
le geste fréquent** — le pas de la file de Revue, répété des centaines de fois par session,
n'a droit à aucun cran (vérifié : aucune classe de la file ne porte de `transition`).

Une seule courbe, `--ease-out`. Une entrée sort de la courbe ; une sortie est linéaire
et plus courte.

### Ce qui ne s'anime jamais

- La **donnée**. Une valeur qui change de chiffre change de chiffre.
- Un verdict, un compte, une durée, un BPM.
- Tout ce qui est peint par un renderer appelé en rafale (progression, watcher, scroll,
  resize). Ces renderers créent leurs nœuds une fois et mutent ensuite — un
  `innerHTML =` dans une boucle d'événements n'anime rien de toute façon, et coûte.

### Ce qui s'anime

`transform` et `opacity`, jamais autre chose. Une transition qui vise `background`,
`width` ou `height` est un défaut de conception : elle force une recomposition.

### Mouvement réduit

Le bloc `@media (prefers-reduced-motion:reduce)` en fin de `styles.css` **met les durées
à zéro, il ne supprime pas les animations**. C'est délibéré : `animationend` doit
continuer à se déclencher, sinon `filing-identify.ts` reste bloqué sur
`.sift-identified-flash` / `.sift-applytags-flash`.

---

## 7. Barre de titre

`decorations: false` (`src-tauri/tauri.conf.json:22`) sur les deux cibles. La barre est
du HTML, haute de `--titlebar-h` (30 px), et c'est le levier principal du rendu macOS
sur Windows.

Deux zones réelles, jamais un dégradé simulé :

- **Gauche** — largeur du rail de navigation, ton du rail
  (`--color-background-tertiary`), bordure droite identique à celle du rail. La ligne
  verticale court sans interruption de la barre de titre jusque dans le rail.
- **Droite** — ton du contenu.

Aucune ligne horizontale n'est ajoutée dans cette barre.

Boutons de fenêtre : carrés alignés à droite hors macOS ; trois pastilles rondes à
gauche sur macOS (`.sift-tb-mac`). Même markup, même câblage — seuls le placement et le
style diffèrent.

**Décision de phase 2 :** le titre de la barre est aujourd'hui le littéral `Sift`
(`chrome.ts:214`), jamais la vue courante. La barre devient porteuse du **titre de la
vue** et fusionne avec la toolbar. Le `.h1` que chaque écran réémet dans son contenu
disparaît, ainsi que le `<h1 class="sift-sr-only">` de `index.html` qui doublonne avec
lui. Géométrie exacte : phase 3.

---

## 8. États systémiques

Définis une fois ici, jamais redécidés par un écran.

| État | Rendu | Règle |
|---|---|---|
| **Repos** | Aucun traitement | Un état confirmé permanent reste neutre |
| **Survol** | `--overlay-hover` | Aplat, jamais une bordure colorée. **Précisé le 2026-09-09 (audit de fin de #24, décision d'Antoine) : un push button n'a PAS de survol** — le kit Big Sur § 02 ne montre que Normal / Disabled pour un push, le survol n'existe que sur les scope buttons et les icon buttons, en aplat. Le survol reste aux rangées, entrées de colonne, rail et boutons discrets (transparents), toujours en `--overlay-hover` ; `filter: brightness()` est retiré de tous les `:hover` |
| **Focus visible** | `outline:2px solid var(--color-border-info); outline-offset:1px` | Sur tout élément interactif, via `:focus-visible`. Une saisie remplace l'anneau par une bordure `--color-text-info` |
| **Sélection** | `--color-background-secondary` | Promue au plan du contenu. Jamais un accent coloré, jamais une bordure latérale |
| **Chargement** | Squelette ou libellé sobre **dans la structure finale** | Ne jamais vider l'écran pour recharger : si des données valides sont affichées, elles restent jusqu'à l'arrivée des nouvelles |
| **Vide** | `emptyStateHtml()` | Titre, note, une action. Distinguer « rien du tout » (impasse, retour vers Revue) de « ce filtre ne rend rien » (les filtres restent à l'écran pour être défaits) |
| **Erreur** | Carte douce, encre `danger`, bouton Réessayer discret | **L'erreur passe avant tout le reste** : après un scan échoué on ne dit rien sur le contenu. Affirmer « aucun doublon » après un échec, c'est affirmer un fait non mesuré |
| **Opération en cours** | Zone de progression + action d'annulation | Patron Utilitaire de disque : cible → action → progression → rapport |

Deux garde-fous qui ne se négocient pas :

- **Jamais `window.confirm()` / `alert()` / `prompt()`.** Un clic synthétique en a déjà
  traversé un et rangé 265 pistes. La confirmation est in-app, **armée et horodatée**
  (`confirm-modal.ts`, `BATCH_CONFIRM_THRESHOLD`). Elle ne vise pas la réversibilité —
  le rangement est annulable — elle vise un clic qui n'est pas humain.
- **Toute donnée non fiable rendue via `innerHTML` passe par `esc()`** (`dom.ts`). Son
  contrat s'arrête au texte et aux valeurs d'attribut **entre guillemets**. Le premier
  `href="${…}"`, attribut non quoté ou donnée dans un `<script>` demande une **seconde**
  fonction, jamais un `esc()` élargi.

---

## 9. Clavier et focus

### État mesuré le 2026-08-19

Quatre raccourcis d'action, tous derrière `if (!state.track) return` (`filing.ts:584`),
donc **Revue uniquement** : Espace, Entrée, ⌫ / X, I. Plus ↑ ↓ dans la file
(`queue-panel.ts:263`). Hors Revue il reste **un** raccourci dans toute l'app : ⌘Z /
Ctrl+Z (`filing.ts:617`). `installNavKeyboard` (`chrome.ts:278`) n'ajoute aucun
raccourci — il réémet un clic sur Entrée/Espace pour un élément **déjà focalisé**.

Sept écrans sur huit exigent la souris, pour une cible qui vit au clavier.

### Décision

**Trois couches, et chacune a une portée nette.**

**Couche 1 — fenêtre.** Disponible partout, quel que soit l'écran.

| Raccourci | Action |
|---|---|
| ⌘/Ctrl + 1…8 | Aller à la n-ième destination du rail |
| ⌘/Ctrl + F | Placer le focus dans la recherche |
| ⌘/Ctrl + , | Réglages |
| ⌘/Ctrl + Z | Annuler la dernière action |
| Échap | Fermer le popover, la modale ou la recherche au premier plan |

**Couche 2 — liste.** Partout où une liste a le focus.

↑ ↓ déplacent la sélection · ⇧+↑↓ étendent · ⌘/Ctrl+A sélectionne tout · Entrée ouvre ·
⌫ écarte · Début/Fin vont aux extrémités.

**Couche 3 — écran.** Les touches propres à un écran, jamais un accélérateur à une
lettre qui écraserait la couche 2. Revue garde Espace (lecture), I (identifier),
Entrée (ranger), ⌫ (écarter).

### Règles de focus

- Un raccourci d'écran ne se déclenche **jamais** quand le focus est dans un `INPUT` ou
  un `TEXTAREA`.
- Un raccourci à une lettre retire le focus du bouton actif avant d'agir, pour que
  Espace n'active pas simultanément le bouton focalisé et la lecture.
- Une modale **piège** le focus (Tab boucle dedans) et le **restitue** à sa fermeture.
- Après un rebuild par `innerHTML`, le focus est perdu : tout écran qui se reconstruit
  sur une action doit reposer le focus explicitement.
- Aucune décision n'est atteignable à la souris seule.

---

## 10. Diff de tokens proposé

Trois entrées seulement. Chacune est justifiée par une duplication ou un manque
mesuré — aucune valeur nouvelle n'est inventée.

### D‑1 · `--rail-w:152px` — nouveau

```diff
+--rail-w:152px;
```

**Preuve.** `152px` est écrit deux fois, dans deux fichiers : `styles.css:417` (`.sb`)
et `chrome.ts:161` (`#sift-tb-left`). Les deux **doivent** rester égales — la bordure
droite du rail continue dans la barre de titre, et un écart d'un pixel casse la ligne
verticale continue. Rien ne le garantit aujourd'hui.

Consommateurs à migrer : les deux ci-dessus. `chrome.ts` construit sa feuille en
JavaScript, elle peut écrire `var(--rail-w)`.

### D‑2 · `--pane-w:272px` — nouveau

```diff
+--pane-w:272px;
```

**Preuve.** `272px` est écrit trois fois : `app.js:39` (`QCOL_DEFAULT`), `app.js:112`
(style inline de `#homequeue`), `styles.css:1759` (`.sift-library-side`). Le
commentaire `styles.css:1754` documente déjà l'intention — « alignée sur la largeur
canonique de `#qcol`/`#homequeue` » — mais en **prose**, jamais en valeur. Sa largeur a
été rattrapée quatre fois de suite (150 → 190 → 245 → 272, `styles.css:1746-1758`) :
une valeur rattrapée quatre fois n'a jamais été dérivée.

`QCOL_DEFAULT` reste une constante JS — elle sert de valeur de repli avant lecture du
CSS. Elle **mire** le token, comme `shared/contracts.ts` mire les structs serde : même
discipline, même geste unique.

### D‑3 · `--duration-slow:250ms` — nouveau

⚠️ **Recalé le 2026-08-20** : le cran existe toujours, mais sa valeur est **300 ms** — la
racine de l'issue #10 (75/150/300, décision d'Antoine du 2026-08-19) prime sur le 250
posé ici sans avoir lu le ticket. Voir § 6.

```diff
--duration-fast:100ms;--duration-base:150ms;--ease-out:cubic-bezier(.2,0,0,1);
+--duration-slow:250ms;
```

**Preuve.** 37 `transition:` dans `styles.css`, 9 durées littérales distinctes, 2 seules
déclarations qui référencent un token. Les trois quarts des littéraux se rangent sous
100 ou 150 ms ; le reste (`.18s`, `.2s`, `.25s`, `.3s`) est un déplacement de matière
— pouce de contrôle segmenté, panneau, barre de progression — qui n'a pas de cran.
Sans lui, la migration des 37 sites est impossible sans écraser une intention.

### Non déclarés volontairement

- **`--toolbar-h`** — la toolbar n'existe pas encore. Le dépôt supprime tout token sans
  consommateur (`--h-32`, `--h-44`, `--h-36`). Il se déclarera en phase 3, avec sa
  première utilisation.
- ~~**`--measure-form:560px`** — le littéral existe (`.sift-settings-stack`) et cette mesure
  de l'issue #9 n'est pas encore déclarée.~~ **Déclarée le 2026-09-09** : premier consommateur
  `.sift-settings-panel` (Réglages, déclinaison #24) ; `.sift-settings-stack` a disparu.

---

## 11. Décisions ouvertes

Quatre points que ce socle ne tranche pas seul.

### O‑1 · Outfit ou une famille plus proche de SF — RENVERSÉ le 2026-08-26, **Inter est retenu** (#34)

**Inter remplace Outfit**, livré par `114325e`. Décision d'Antoine en session : « je préfère
inter comme police ». Elle renverse la conclusion du 2026-08-16 et rejoint O‑1 de la skill
`sift-macos-ui`, qui disait « Inter remplace Outfit » depuis le 2026-08-19 : la skill avait
donc raison, mais pour une raison qu'elle n'avait pas — le choix, pas la mesure de contraste.
Sites touchés : `package.json` + lock, les trois imports de `frontend/main.ts`, `--font-ui`
ci-dessus, et la table `PAQUET` de `test/font-weights.test.ts`, qui vérifie que les trois
faces existent.

**Ce que disait la décision de 2026-08-16, conservé parce que la mesure reste vraie** : le
défaut relevé alors n'était pas la famille mais le bundle — `main.ts` n'importait que trois des
neuf faces livrées, et le contraste manquant vivait dans la face 700 jamais chargée (400→700 =
+3,67 % de largeur, plus que le 400→600 de Segoe UI). La table de styles macOS d'Apple reste
adoptée verbatim (#31). Ce diagnostic n'est pas infirmé ; il a cessé d'être le critère.

⚠️ **Ce que la bascule périme, et qui a été re-mesuré le 2026-08-26** — toute largeur du dépôt
dérivée d'une mesure faite sous Outfit :

| Largeur | Sous Outfit | Sous Inter | Suite |
|---|---|---|---|
| Colonne verdict (`AUTHENTIQUE` + pastille 6 + gap 4) | 81,67 → **92** | 83,56 → **93,56** | **96** (multiple de 4 supérieur) |
| `--rail-w` (item le plus long + badge) | — | 66,77 + 25,09 = 91,86 pour 131 dispo | **200 inchangé**, 39 px de marge |

Et le résultat qui vaut règle : **Inter n'est pas « plus large » qu'Outfit**, il est plus large
en CAPITALES et plus étroit en minuscules — « Bibliothèque » RÉTRÉCIT (73,7 → 70,57) pendant que
« MUSIQUE A TRIER » gagne 14,6 % (98,98 → 113,39). Une largeur dérivée d'une mesure **ne se
convertit pas par un facteur** : elle se re-mesure libellé par libellé, sur le texte que l'écran
porte vraiment. Les deux commentaires concernés de `styles.css` le portent.

Chronologie complète sur #34.

### O‑2 · L'encre reste chaude sur des fonds devenus froids — RÉSOLU le 2026-08-19 (#37)

**L'encre neutre a rejoint la famille froide des fonds** — variante B de #37, appliquée
par `5a6eae4` : H 286.3, chroma 0,0067 uniforme sur les huit tokens de texte, clarté
strictement inchangée (c'est elle qui porte le contraste), dans les trois blocs. Le
commentaire « question laissée ouverte » de `styles.css` est réécrit en décision datée
(`styles.css:38`). Les valeurs listées par la skill `sift-macos-ui` (chromas
différenciés, H 286.2) décrivent une variante antérieure : le code et #37 font foi.

### O‑3 · Le plafond de 560 px

`.sift-settings-stack{max-width:560px}` laisse 44 % de la fenêtre vide sur Réglages et
Clé USB (fenêtre 1200 px, moins le rail 152 et le padding 2×24 : 1000 px utiles).

**La correction n'est pas d'élargir la colonne.** Réglages Système de macOS emploie
justement un panneau étroit — mais à côté d'une **sidebar de catégories**. Ce qui manque
n'est pas de la largeur, c'est la seconde colonne. Décision de phase 3.

**Soldé le 2026-09-09** (déclinaison #24) : Clé USB est lu contre Utilitaire de disque
(colonne des disques + zone principale bornée à `--measure-data`), Réglages contre Réglages
Système (colonne des catégories + panneau borné à `--measure-form`, token déclaré ce jour).
`.sift-settings-stack` n'existe plus.

### O‑4 · Le signal orange→vert décrit par le brief n'existe pas

Le brief de la skill présente un « signal de compatibilité orange→vert » comme identité
visuelle du produit, à ne jamais affaiblir. **Vérifié le 2026-08-19 : il n'existe pas.**
`frontend/styles.css` contient deux occurrences de `gradient`, toutes deux sur
`.sift-qresize` (la poignée de redimensionnement), et c'est un dégradé vertical
transparent → gris → transparent, sans rapport.

Le signal réel de Sift est **discret**, pas continu : des puces de verdict sur quatre
teintes sémantiques, avec un libellé texte (`LOSSLESS`, `FAKE`, `DUPLICATE`) qui rattrape
la couleur pour un daltonien. C'est meilleur qu'un dégradé pour ce travail — un dégradé
suggère un continuum là où le verdict est catégoriel.

**Aucun dégradé ne sera introduit sur cette base.** Si un signal continu est voulu, c'est
une décision produit à prendre explicitement, pas un existant à préserver.

---

## 12. Jargon conservé

Volontairement en anglais dans l'interface, ne pas « corriger » :

`LOSSLESS` (le rail, jamais le mot de verdict depuis le 2026-09-10) · `DUPLICATE` · `MATCH` ·
`CHECK MATCH` · `kbps` · `kHz` · `MP3` · `AIFF` · `WAV` — `FAKE` en est sorti le 2026-09-10 :
le verdict dit `FAUX`.

Ce n'est pas du jargon d'implémentation : c'est le vocabulaire professionnel de la
cible. Traduire dégraderait la reconnaissance.

---

## 13. Index des specs d'écran

Une ligne par écran. Le contenu vit dans `docs/ui-specs/`, jamais ici.

| Surface | Spec | Statut |
|---|---|---|
| **Rail** (zone B, tous écrans) | [`docs/ui-specs/rail.md`](docs/ui-specs/rail.md) | écrite |
| Bibliothèque — *table canonique* | [`docs/ui-specs/bibliotheque.md`](docs/ui-specs/bibliotheque.md) | écrite |
| Revue | [`docs/ui-specs/revue.md`](docs/ui-specs/revue.md) | écrite |
| Journal | [`docs/ui-specs/journal.md`](docs/ui-specs/journal.md) | écrite |
| Rekordbox | [`docs/ui-specs/rekordbox.md`](docs/ui-specs/rekordbox.md) | écrite |
| Clé USB | [`docs/ui-specs/cle-usb.md`](docs/ui-specs/cle-usb.md) | écrite |
| Réglages | [`docs/ui-specs/reglages.md`](docs/ui-specs/reglages.md) | écrite |

Les deux vues absorbées ne sont pas orphelines, elles ont une destination écrite :

| Vue absorbée | Où son contenu vit désormais |
|---|---|
| **Accueil** | `rail.md` (section Sources) · la porte de racine manquante remonte dans la barre unifiée, voir `reglages.md` |
| **Écartés** | `bibliotheque.md` — « À re-sourcer » et « Corbeille », deux sources de la même table |

⚠️ `docs/` est en liste blanche. `docs/ui-specs/` est ré-autorisé par `!docs/ui-specs/`
dans `.gitignore` — sans cette ligne le dossier n'existerait pas pour git, en silence.

Patron macOS de chacun : § 15. Ordre d'écriture : § 17.

---

# Phase 3 — Shell, mapping, table

## 14. Le shell unique

### La règle qui remplace les deux grammaires

Aujourd'hui l'app porte **deux grammaires incompatibles** : Revue et Accueil sont en
panneaux fixes sans défilement de page ; les six autres écrans passent par `block()`
(`app.js:33`) et laissent la page entière défiler. Le modèle mental change à chaque
changement d'écran.

Une seule règle les remplace :

> **Trois zones. Une seule flexe — celle qui porte l'objet d'attention. Les deux
> autres sont à largeur fixe. La page ne défile jamais ; chaque zone défile chez elle.**

C'est la règle de macOS lui-même, pas une invention : dans Finder la liste flexe et
l'inspecteur est fixe ; dans un éditeur de photo le canevas flexe et la palette d'outils
est fixe. La zone qui flexe est celle qu'on regarde.

### Les quatre zones

```
┌──────────────────────────────────────────────────────────────────┐
│ A · Barre unifiée        --toolbar-h                             │
├──────────┬───────────────────────────────┬───────────────────────┤
│          │                               │                       │
│ B · Rail │  C · Zone centrale            │  D · Inspecteur       │
│ --rail-w │                               │  --pane-w             │
│          │                               │                       │
│  fixe    │                               │  fixe, repliable      │
│ repliable│                               │                       │
└──────────┴───────────────────────────────┴───────────────────────┘
```

#### A — Barre unifiée (titlebar + toolbar fusionnées)

Une seule barre, jamais deux empilées. Elle absorbe l'actuelle barre de titre de 30 px.

De gauche à droite : contrôles de fenêtre (à gauche sur macOS, à droite ailleurs) ·
**titre de la vue courante** · actions contextuelles de la vue, deux ou trois au
maximum · modes de vue · **recherche, toujours à droite**.

Icônes seules avec infobulle, jamais de libellé sous une icône. Les espaces vides sont
zone de drag.

**Hauteur — dérivée, pas choisie.** La barre vaut le contrôle le plus haut qu'elle
porte, plus un cran d'espacement au-dessus et au-dessous. Le contrôle le plus haut est
le champ de recherche : `--text-md` sur une ligne, plus son padding vertical de 6 px,
soit ~29 px calculés depuis `styles.css`. D'où `--toolbar-h: 29 + 8 + 8 ≈ 44px`.

⚠️ `--titlebar-h` a un consommateur au-delà de `chrome.ts` : le bandeau de mise à jour
s'y positionne. Le passage de 30 à 44 doit le suivre dans le même geste.

#### B — Rail de navigation

Fixe, repliable. Porte la **teinte de chrome** (`--color-background-tertiary`), la même
que la zone gauche de la barre unifiée : la bordure verticale court sans interruption
de la barre jusqu'en bas du rail.

**Largeur — dérivée.** Le libellé le plus long est « Bibliothèque ». À `--text-base`,
plus l'icône (17 px), le gap (`--space-12`), le padding horizontal (`--space-8` × 2) et
un badge de compte à droite (~26 px + `--space-8`), le total dépasse les 152 px
actuels — ce qui explique qu'un seul item porte un badge aujourd'hui. `--rail-w: 200px`,
premier multiple de 8 qui loge l'ensemble, et la valeur tombe dans la bande 200–260 px
des apps système. **À confirmer par une mesure dans la vraie fenêtre avant
implémentation** : la largeur d'un texte ne se déduit pas d'un calcul.

Contenu groupé par sections, en-têtes en petites capitales discrètes, **un seul niveau
d'indentation**. Item actif : fond plein arrondi (`--color-background-secondary`),
jamais de bordure ni de barre latérale colorée.

⚠️ **Conflit tranché, à ne pas « corriger » à la prochaine passe.** Le kit Big Sur
(§ 06-Sidebars) montre l'item actif d'une sidebar en **bleu accent plein, texte blanc**.
Ce fichier l'interdit — ici même, et au § 8 : « Sélection : jamais un accent coloré ».
La précédence tranche (ce fichier avant le kit), et le motif tient : dans Sift, le bleu
est déjà pris par une sémantique (« interactif, focus, sélection, lien ») et par l'aplat
d'accent des boutons primaires. Un rail bleu ferait du rail la chose la plus accentuée
d'un écran dont le sujet est la table. Le rail garde son gris.

Replié, le rail garde ses icônes et perd ses libellés — il ne disparaît pas.
Raccourci de bascule : proposition ⌥⌘S sur macOS (convention Finder), Ctrl+B ailleurs.
**Marqué proposition — à vérifier dans les HIG avant d'être figé.**

#### C — Zone centrale

C'est presque toujours **la table** (§ 16). Deux exceptions, et elles sont nommées :
la surface de travail de Revue, et le panneau de formulaire de Réglages.

Défile chez elle, jamais en emportant les autres zones. Le champ de recherche, les
filtres et les en-têtes de colonne restent visibles quand la liste défile.

#### D — Inspecteur

Fixe à `--pane-w`, repliable. Affiche le détail de la **sélection courante**.

Sélection multiple : un **résumé agrégé** (nombre, formats, durée totale, actions
possibles), jamais un état vide. Sections repliables à en-têtes discrets ; libellé à
gauche en `--color-text-secondary`, valeur à droite en `--color-text-primary`.

**L'inspecteur n'est jamais un bloc dans le flux.** Aujourd'hui le détail de
Bibliothèque est rendu dans `#bibplayer`, placé après la liste et après la section
doublons : ouvrir une piste au rang 300 pousse son détail hors de l'écran.

### Les deux profils

Même trois zones, même règle. Ce qui change est **laquelle flexe**.

| Profil | Zone fixe gauche | Zone qui flexe | Zone fixe droite | Objet d'attention |
|---|---|---|---|---|
| **Parcours** | rail | table centrale | inspecteur | La liste |
| **Poste de décision** | rail + file (`--pane-w`) | surface de travail | — | La piste ouverte |

Le profil Poste de décision existe pour une raison mesurable, pas par confort : la
surface de travail de Revue porte le spectrogramme, borné à `--measure-data` (1200 px)
et **dupliqué côté Rust** dans `analysis::spectrum::MAX_COLS`. Une zone de largeur fixe
y présenterait de la donnée étirée ou tronquée — c'est-à-dire fausse, dans une app dont
le métier est de détecter du faux.

### Redimensionnement et persistance

- Rail : repliable, largeur non redimensionnable (elle est dérivée du contenu).
- File de Revue : redimensionnable, bornes 220–480 px, valeur persistée. Le mécanisme
  existe déjà (`app.js:39`) et ne change pas.
- Inspecteur : repliable, largeur non redimensionnable.
- Toute largeur persistée est relue au montage, jamais gardée dans une variable JS
  vivante — chaque changement d'écran reconstruit la zone.

---

## 15. Mapping des vues

Aucune vue orpheline. Deux fusions proposées, chacune avec son motif.

| Vue | Patron macOS | Profil | Zone C | Zone D |
|---|---|---|---|---|
| **Bibliothèque** | Finder / Music | Parcours | Table des pistes rangées | Détail de la piste |
| **Journal** | Console | Parcours | Table des actions, groupées par session | Détail de l'entrée + annuler |
| **Revue** | Finder + Utilitaire de disque (mode Batch) | Poste de décision | Surface de travail : lecture, verdict, identification, rangement | — (la file tient lieu de zone fixe) |
| **Rekordbox** | Utilitaire de disque | Parcours | Liste des candidats de la section choisie | Détail du candidat |
| **Clé USB** | Utilitaire de disque | Parcours | Le disque choisi : tête, occupation, faits, actions (colonne B′ = liste des disques, au plan de la file — 2026-09-09, #24) | — (pas d'inspecteur : le disque est la zone C) |
| **Réglages** | Réglages Système | Parcours | Panneau du réglage choisi, borné à `--measure-form` | — (les catégories occupent la zone gauche) |

### Fusion 1 — Accueil disparaît dans le rail

**Motif.** Accueil ne montre qu'une chose : les dossiers surveillés et leur état de
scan. Dans Finder, une source n'est pas un écran — c'est une **entrée de sidebar**. Un
écran entier pour lister des sources est un détour : l'utilisateur y va pour en ajouter
une, puis en repart.

**Sort.** Les dossiers surveillés deviennent une section du rail (« Sources »), avec
leur pastille de couleur, leur compte de nouveaux fichiers et leur bascule de
surveillance au clic droit. Le bouton « ajouter un dossier » vit au pied de la section.
Cliquer une source **filtre Revue** sur ses fichiers.

**Ce que la fusion coûte.** La porte de premier réglage (racine de bibliothèque non
définie) perd son emplacement. Elle remonte dans la barre unifiée, comme bandeau
persistant tant que la racine manque — ce qui la rend d'ailleurs visible depuis tous
les écrans, pas seulement depuis celui qu'on quitte.

> ⚠️ **ÉCART NOMMÉ — ce paragraphe est périmé depuis le 2026-09-02 (issue #54).** Le
> bandeau de la barre unifiée (`#sift-gate`) est **supprimé**, code et feuille compris.
> Cause : la racine a cessé d'être un prérequis de la conversion — on convertit en place
> ou vers un dossier externe sans elle — donc le bandeau criait sur tous les écrans un
> prérequis qui ne mord qu'à une destination de l'arbre. Le rappel est descendu dans le
> **rail**, sous la section Sources (carte ambre `.sift-railwarn`, direction A2 du
> ticket). Le tracker prime sur ce fichier (`CLAUDE.md` § Front) : la décision vivante
> est l'issue #54, et l'écart est nommé des deux côtés — voir `docs/ui-specs/rail.md`
> § États et `docs/ui-specs/reglages.md` § États.

### Fusion 2 — RÉFUTÉE PAR LA MESURE, le 2026-08-19

Ce qui suit était la proposition. Elle repose sur « Écartés est une vue filtrée de la même
donnée », et **c'est faux** : `EcarteItem` porte **8 champs** contre **16** pour `LibraryTrack`
(`shared/contracts.ts`). Ni BPM, ni durée, ni format, ni genre, ni année, ni pochette. Les six
colonnes de la table rendraient donc cinq tirets sur six pour chaque ligne écartée.

Et l'écran porte **sept affordances** que la table n'a pas : `store` (liens boutiques pour
racheter la piste), `copy-query`, `requeue`, `restore`, `retry`, `trash`, `purge`. Ce sont elles
qui font l'écran — le fondre dans la table les perdrait ou obligerait à les y ajouter, donc à
transporter l'écran dans la table plutôt que l'inverse.

**Écartés reste une destination.** La moitié utile de la fusion a été faite : il vit dans la
section « Bibliothèque » du rail, à côté de « Rangés », donc la parenté se lit dans la navigation.
Ce qui manquait n'était pas la fusion, c'était le groupement.

Réouvrir cette décision demanderait d'abord d'enrichir `EcarteItem`, ce qui est un chantier
backend, pas de design.

### Proposition d'origine, conservée pour l'historique

**Motif.** Écartés est une **vue filtrée de la même donnée** : des pistes, avec un
statut. Finder ne fait pas un écran pour la Corbeille — c'est un item de sidebar qui
change le contenu de la même table.

**Sort.** Deux entrées dans la section « Bibliothèque » du rail : « À re-sourcer » et
« Corbeille ». Même table, mêmes colonnes, colonne Verdict qui porte le motif d'écart
(tronqué, faux, doublon). Les liens boutiques et l'action « purger » deviennent des
actions de la barre unifiée quand ces sources sont actives.

**Ce que la fusion coûte.** Rien de mesuré. Écartés a aujourd'hui ses propres hauteurs
de ligne (58 px et 42 px, `ecartes-view.ts:162,173`) : elles rejoignent celle de la
table, ce qui est le but.

### Bilan

**Huit destinations deviennent six.** Le rail passe de huit items plats à trois
sections : Traiter (Revue · Journal) · Bibliothèque (Rangés · À re-sourcer · Corbeille) ·
Exporter (Rekordbox · Clé USB), plus Réglages au pied. Les sources surveillées forment
une quatrième section, au-dessus.

### Sous-modes

- **Revue / Batch** — ce n'est pas un écran mais un **changement de zone C** : la surface
  de travail devient le **résumé de la sélection** (compte, verdicts, totaux, « Ranger la
  sélection »). La file reste, le rail reste, la barre reste. ⚠️ **Révisé le 2026-08-21,
  complété le 2026-08-24** : pour Revue, la recherche reste dans la **colonne file**,
  désormais **en tête** (décision E, patron HIG d'une liste) ; le segmenté **Détail / Lot
  est retiré**, le mode **Batch** s'arme par une **icône de sélection dans la barre**
  (§ 11 option 4, patron « Sélectionner » Photos) qui fait apparaître les **cases par
  ligne** dans la file — plus de table à part. **Deux écarts assumés à la règle « modes de vue
  dans la barre » (§ 14 A), tous deux locaux à Revue**, reconfirmés le 2026-08-25 : la
  recherche n'est **pas** dans la barre unifiée, et Revue n'a **plus aucun contrôle de mode de
  vue** — ni dans la barre, ni dans la colonne file, puisque le segmenté Détail / Lot est
  retiré et que le sous-mode Batch s'arme par une **action** (l'icône de sélection), pas par
  un sélecteur de mode. Bibliothèque Table/Grille garde sa bascule dans la barre. Détail :
  `docs/ui-specs/revue.md` § Zone A / B′.
- **Bibliothèque / Table et Grille** — deux rendus de la même zone C, bascule dans la
  barre unifiée. Le tri est partagé entre les deux.
- **Rekordbox / 4 sections M8** — les quatre sections deviennent quatre **entrées de la
  zone gauche**, plus un item « Tout ». La zone C montre les candidats de celle qui est
  choisie. Cela résout l'empilement vertical de quatre cartes concurrentes.

---

## 16. La table centrale

L'écran de vie du DJ. Une seule table, un seul comportement, partout où il y a des
pistes : Bibliothèque, À re-sourcer, Corbeille, file de Revue, mode Batch.

### Colonnes par défaut

| # | Colonne | Largeur | Rendu |
|---|---|---|---|
| 1 | **Verdict** | fixe | Pastille + libellé (§ « Signal » ci-dessous) |
| 2 | Pochette | 44 px | Vignette, repli sur une icône de vinyle |
| 3 | **Artiste** | flex 1.4 | `--color-text-primary` |
| 4 | **Titre** | flex 1.4 | `--color-text-primary` |
| 5 | **BPM** | fixe | `--font-mono`, `tabular-nums`, aligné à droite |
| 6 | **Durée** | fixe | `--font-mono`, `tabular-nums`, aligné à droite |
| 7 | Genre | flex 1 | `--color-text-secondary` |
| 8 | Année | flex 0.6 | `--font-mono`, `tabular-nums` |
| 9 | Qualité | fixe | Pastille format + débit |

**BPM et Durée sont des ajouts, et ce sont les deux plus importants.** Les deux champs
existent déjà dans le contrat (`shared/contracts.ts:313-314`, `duration` et `bpm`), et
**aucun des deux n'est affiché** : la table actuelle trie sur Artiste, Titre, Genre,
Année (`library-views.ts:48-53`). Un DJ trie sa bibliothèque par tempo. L'information
est en base et n'atteint pas l'écran.

⚠️ **Tonalité et énergie n'existent nulle part.** Vérifié le 2026-08-19 : aucun champ
de tonalité dans `shared/contracts.ts`, aucune colonne dans `db.rs` (le seul `key TEXT`
est la clé de la table `settings`). Le brief de la skill dit que la cible pense en
« BPM, tonalité, énergie » : un tiers de cette phrase est affichable aujourd'hui, un
tiers l'est après ce changement, et le dernier tiers **n'est pas une décision de
design** — c'est de l'analyse à écrire côté Rust. Aucune colonne fantôme ne sera
spécifiée pour du vide.

### Tri, largeur, ordre

- **Toutes** les colonnes sont triables au clic sur l'en-tête, indicateur de direction
  visible, `aria-sort` tenu à jour. Aujourd'hui quatre le sont.
- Largeurs redimensionnables au glisser sur le séparateur d'en-tête, **mémorisées**.
  *Livré le 2026-08-19* (`frontend/library-columns.ts`). Source Apple : HIG « Lists and tables »
  § macOS, « Let people resize columns ». Bornes 48–600 px, `localStorage`, et une colonne
  non touchée garde sa règle CSS — c'est ce qui lui permet de continuer à suivre la largeur
  de la zone.
- Colonnes réordonnables au glisser, ordre mémorisé. *Livré le même jour.* Pas de source
  Apple : la page HIG ne parle que de réordonner des **lignes**. Cette règle vient d'ici,
  et se marque comme telle.
- **Un en-tête porte deux gestes.** Bouton de tri et poignée de déplacement sur le même
  élément, séparés par un seuil de 5 px de déplacement — sans lui, réordonner trierait
  aussi, deux effets pour un geste.
- ⚠️ La même page HIG conseille « Consider using alternating row colors ». Le § Densité
  ci-dessous l'interdit. Conflit tranché par la précédence (ce fichier avant les HIG), et
  l'argument d'Apple ne mord pas : il vise une table **large**, celle de Sift vit entre un
  rail et un inspecteur.
- Le tri est **partagé** entre Table et Grille, et **stable** : il ne se rejoue jamais
  sur un tick de données, seulement sur une action utilisateur.

### Densité

**Une seule hauteur de ligne pour toute l'app.** Aujourd'hui il y en a quatre —
34 px en Bibliothèque, 150 px en grille, 58 px et 42 px en Écartés
(`bibliotheque-view.ts:455,468`, `ecartes-view.ts:162,173`).

`--row-h: 32px`, dérivé : vignette de pochette 24 px (le plus petit format où une
pochette reste reconnaissable) plus `--space-4` au-dessus et au-dessous. C'est au-dessus
de la bande 24–28 px d'une liste Finder, et la raison est nommée : la ligne de Sift
porte une pochette, celle de Finder n'en porte pas.

~~Pas d'alternance de fond de ligne. La séparation vient de l'espace, pas d'un trait ni
d'un zébrage.~~ **Renversé le 2026-09-10** (décision d'Antoine, « pas fan du design de la
liste, comment ferait Apple ? ») : la rangée est lue contre la présentation par liste du
Finder — la seule table de macOS, le kit Big Sur n'en a aucune. **Fonds alternés**
(`--overlay-alt`, un cran au-dessus de `--overlay-hover`, parité d'index posée par le rendu
virtualisé — jamais `:nth-child`, qui s'inverse au défilement), **aucun filet entre
rangées, aucun arrondi, aucun survol de rangée** (le triangle de lecture se révèle au
survol de la pochette seule), sélection pleine largeur. Le format devient une colonne de
texte, plus une pastille. La règle d'avant n'était de toute façon pas appliquée : le code
traçait un filet sous chaque rangée ET un arrondi de 7 px. Le § Tri ci-dessus, qui
« tranchait » le conflit avec les HIG par la précédence, est caduc sur ce point.

### Signal de compatibilité — une seule forme, partout

Le verdict est **catégoriel**, pas continu. Il se rend en **pastille pleine + libellé
texte**, dans la colonne 1, identique dans les cinq tables.

| Verdict | Teinte | Libellé |
|---|---|---|
| Authentique (tout rail) | `success` | `VRAI` |
| Faux lossless | `danger` | `FAUX` |
| Douteux | `warning` | `À VÉRIFIER` |
| Non analysé | neutre | `—` |

**Recalé le 2026-09-10** (Antoine : « le verdict annonce "vrai" ou "faux", pas "lossless" »).
Jusque-là : `LOSSLESS` (authentique, rail lossless), `AUTHENTIQUE` (authentique, rail lossy),
`FAKE`. Le verdict répond à une seule question — le fichier est-il ce qu'il prétend être — et
son mot dit la réponse, pas le rail ; le rail reste dit à côté, par la ligne de format. `FAKE`
quitte l'allowlist de jargon ci-dessous ; `LOSSLESS` y reste pour le rail, les facettes et les
chips, plus pour le mot de verdict.

⚠️ **Révisé le 2026-08-19 contre les littéraux réels du backend** (`worker.rs::verdict_str` :
`ok` / `fake` / `grey`, plus NULL). Deux corrections mesurées, pas des choix : **`DUPLICATE` n'est
atteignable par aucune valeur de `tracks.verdict`** — un doublon sort du scan de dédoublonnage
(`DupGroup`) et se rend en mode Lot et en Revue, pas dans cette colonne ; et `ok` sur un fichier
lossy ne peut pas s'écrire `LOSSLESS` sans mentir — le libellé reprend le vocabulaire déjà présent
(`report-view.ts` « qualité authentique », `queue-panel.ts::verdictWord` « à vérifier »).
`LOSSLESS` exige les deux faits, comme `qualityChipTone` : verdict sain ET rail lossless.

Le libellé texte n'est pas décoratif : c'est lui qui rattrape la couleur pour un
utilisateur daltonien, et les HIG interdisent de porter un état par la seule couleur.
Il ne descend jamais sous `--text-xs` (10 px) et ne s'atténue jamais à l'opacité —
voir § 4, « l'échec est l'information qu'on n'a pas le droit d'estomper ».

**Aucun dégradé.** Un dégradé suggère un continuum là où le jugement est discret. Voir
§ 11, O‑4.

### Sélection multiple

Standard système, et **elle n'existe pas aujourd'hui** en Bibliothèque (les lignes sont
`data-bib="row"`, ouverture simple ; seul le mode Lot a des cases).

Clic = sélectionner · ⇧+clic = étendre la plage · ⌘/Ctrl+clic = ajouter ou retirer ·
⌘/Ctrl+A = tout · ↑ ↓ = déplacer · ⇧+↑↓ = étendre · Entrée = action principale ·
⌫ = écarter · Début / Fin = extrémités.

La sélection survit au tri et au changement de filtre tant que les lignes restent
visibles. L'inspecteur montre le résumé agrégé.

### Menu contextuel

**Il n'y en a aucun dans toute l'app** — vérifié le 2026-08-19, zéro gestionnaire
`contextmenu` dans `frontend/`. C'est le manque le plus coûteux de la table : chaque
action secondaire est aujourd'hui un bouton dans la ligne, et chaque bouton mange de la
largeur sur 15k lignes pour être utilisé sur une.

Le clic droit ouvre les actions secondaires : Ouvrir l'emplacement · Identifier ·
Fiche Discogs · Réanalyser · Changer la destination · Écarter · Restaurer.

**Reste dans la ligne :** le bouton lecture, et lui seul. C'est le geste primaire.

### Temps réel

Les mises à jour ne provoquent **aucun saut de layout** : largeurs stables, nombres en
chiffres tabulaires alignés à droite. Aucune animation sur une valeur qui change
souvent. Un renderer appelé en rafale crée ses nœuds une fois et mute ensuite.

---

## 17. Ordre d'implémentation

Du plus structurant au plus cosmétique. Chaque étape est livrable seule et laisse l'app
utilisable.

> **État au 2026-08-19.** Les dix étapes livrées et vérifiées dans la vraie fenêtre. Ce qui reste est nommé sous le tableau — pas « presque fini », la liste exacte.

| # | Étape | État | Débloque | Frictions closes |
|---|---|---|---|---|
| 1 | **Sortir la maquette du chemin de production** — `router.ts`, `app.js` hors Tauri | ✅ livrée | Tout le reste | F11 |
| 2 | **Barre unifiée** — `--toolbar-h`, titre de vue, recherche à droite | ✅ livrée | Recherche, actions, titre unique | F2, F9 |
| 3 | **Shell à trois zones** — plus de `block()`, inspecteur hors du flux | ✅ livrée | Le mapping entier | F1, F5, F6 |
| 4 | **Rail restructuré** — 3 sections + Sources, repli, fusion 1 | ✅ livrée | Navigation lisible, Accueil absorbé | F3 (accès) |
| 5 | **Table unique** — BPM, Durée, `--row-h`, menu contextuel, sélection multiple | ✅ livrée² | La vie quotidienne du DJ | § 16 |
| 6 | **Clavier couche 1** — ⌘1…8, ⌘F, ⌘,, ⌘B, Échap | ✅ livrée | L'usage sans souris | F4 |
| 7 | **Grammaire de boîte à deux niveaux** — `outline` retirée | ✅ livrée | Cohérence de surface | F8 |
| 8 | **Motion unifiée** — 54 durées sur trois crans | ✅ livrée | — | § 6 |
| 9 | **Réglages en deux colonnes** | ✅ livrée | — | F7, O‑3 |
| 10 | **Rekordbox en quatre entrées** | ✅ livrée¹ | — | F10 |

² Complète depuis le 2026-08-19 : navigation clavier de la table, puis actions de masse dans le
menu contextuel (Réanalyser · Écarter · Corbeille). Voir « Ce qui reste vraiment » pour ce qui n'a
pas été exécuté contre la base réelle.

¹ Vue tourner le 2026-08-19 sur un XML **réellement lié** (24 playlists, 2828 pistes) : cinq
entrées peintes, sélection exclusive, zone C qui suit. Reste non vérifié le rendu **avec des
candidats en attente** — les cinq comptes valent 0.

### Ce qui reste

**Fusion 1 — faite.** Accueil a disparu dans le rail : les dossiers surveillés y sont une section,
cliquer l'un d'eux filtre Revue, et la porte de racine manquante est remontée au niveau fenêtre.

**Fusion 2 — réfutée, pas reportée.** Voir § 15 : la mesure a montré que les deux écrans ne
portent pas la même donnée. Écartés reste une destination, groupée avec Rangés dans le rail.

**Sélection multiple — faite.** Clic, ⇧+clic, ⌘/Ctrl+clic, ⌘/Ctrl+A, et résumé agrégé dans la
zone D.

**Couche 2 du clavier — faite le 2026-08-19.** ↑ ↓ déplacent, ⇧+↑↓ étendent, Début/Fin vont aux
extrémités (`bibliotheque-view.ts::stepBibSelection`). Le déplacement se fait par **index** dans la
liste ordonnée et jamais en marchant sur les nœuds du DOM : la table est virtualisée, un parcours
du DOM s'arrêterait au bord de ce qui se trouve rendu. Une version antérieure de cette section
disait cette navigation impossible faute de gestion de focus de liste — c'était une conclusion
tirée du mécanisme envisagé, pas de la contrainte.

### Ce qui reste vraiment

**Actions de masse — faites le 2026-08-19.** Trois actions passent à N, et les trois sont
**mesurées contre le contrat IPC**, pas choisies : Réanalyser (`reanalyze_tracks` prend déjà un
tableau), Écarter (`reject_batch`, l'IPC du mode Lot), Corbeille (`trash_track` unitaire, bouclé
séquentiellement — le backend sérialise de toute façon derrière son Mutex). Identifier n'y est pas,
et c'est une décision : chaque identification demande de **choisir** un candidat. Le menu garde la
même liste d'entrées aux mêmes positions quelle que soit la taille de la sélection ; ce qui ne
s'applique pas est désactivé, jamais retiré. Motifs et tableau complet :
`docs/ui-specs/bibliotheque.md` § Décisions du 2026-08-19.

Vérifié dans la vraie fenêtre : sélection de 3 par ⇧+clic → les deux entrées singulières
désactivées, les trois autres portant `(3)`, résumé agrégé en zone D. Clic droit **hors** sélection
→ la sélection tombe à la ligne visée et le menu repasse en unitaire. **Non exécuté** : aucune des
trois actions n'a été lancée contre la base réelle — écarter ou jeter de vraies pistes n'est pas
une vérification à prendre seul.

**Rekordbox à quatre entrées : vu tourner le 2026-08-19.** L'affirmation précédente — « cette
machine n'a aucun XML lié » — était **fausse** : `C:\Users\LEETJ\Documents\rekordbox\library.xml`
est lié, 24 playlists et 2828 pistes. Les cinq entrées (Tout · Fichiers · Métadonnées · Pochettes ·
Playlists) sont peintes, la sélection est exclusive (`all*` → `files*`) et la zone C passe bien des
quatre sections à la seule choisie. Ce qui n'a **pas** été vu est le rendu avec des candidats en
attente : les cinq comptes valent 0, tout est « à jour ». Table de candidats, inspecteur de
candidat, sheet de progression et rapport restent donc non vérifiés — et le devenir demande une
divergence réelle entre Sift et Rekordbox, pas une manipulation à fabriquer.

**Colonnes redimensionnables et réordonnables — livrées le 2026-08-19.** `frontend/library-columns.ts`,
plus un menu contextuel d'en-tête (patron Finder) qui porte « Réinitialiser les colonnes ». Vérifié
dans la vraie fenêtre : glissement d'un séparateur, glissement d'un en-tête, tri **inchangé** après
un déplacement, réinitialisation qui vide le stockage. Deux planchers, et il en fallait deux : le
token `--col-min-w` empêche une colonne VOISINE de s'écraser (mesuré, Genre tombait à 35 px avec le
seul garde JS), le plafond dynamique de `startResize` empêche la ligne de déborder. Après correctif,
un glissement de 900 px donne Artiste 545, Titre 48, Genre 48, et `scrollWidth == clientWidth`.

**« Ouvrir l'emplacement » — livré le 2026-08-19.** Commande Rust `reveal_track` : prend un
`track_id`, jamais un chemin, parce que la branche Windows lance un processus. Câblée sur la
première entrée du menu et sur le double-clic. Chemin nominal exécuté dans la vraie fenêtre, et le
chemin d'erreur aussi — il a d'ailleurs trouvé une piste de la base dont le fichier n'existe plus
sur le disque, ce que la commande dit en toutes lettres au lieu d'ouvrir un dossier au hasard.

**Débordement de l'inspecteur — corrigé le 2026-08-19, et la cause n'était pas celle qu'on
cherchait.** `.sift-report-scroll` annonçait 369 px pour 287 alors que chacun de ses enfants
directs mesurait 287, parce que le coupable ne débordait pas par sa **largeur** mais par sa
**position** : `.sift-player-controls` porte `margin-left:82px` (alignement sur la forme d'onde,
conçu pour la surface large de Revue) et la surcharge de zone D lui donnait `width:100%` — 287 de
large à partir de 82, donc un bord droit à 369. Un filtre `scrollWidth > clientWidth` ne pouvait
pas le voir ; il fallait comparer les **bords droits**. Correctif : `margin-left:0` dans le seul
bloc `#sift-aside` (`styles.css`). Mesure après : `scrollWidth` 287 sur les deux conteneurs, zéro
élément dépassant à droite, et `.sift-volume-block` toujours replié à 20 px au repos.

**~~`.sift-volume-track` déborde et ce n'est PAS un défaut~~ — PÉRIMÉ le 2026-08-25.** Ce paragraphe
protégeait un mécanisme qui n'existe plus : le repli-au-survol a été **retiré** (Antoine, 2026-08-25,
« le design n'est pas bon du tout »), `.sift-volume-block` avec lui, et le volume est désormais la
**capsule du kit Big Sur, toujours visible**, inlinée telle quelle en SVG (`.sift-volume` dans
`report-view.ts`). Il n'y a donc plus ni bloc replié à 20 px au repos, ni débordement à ne pas
corriger. La note reste ici **barrée plutôt que supprimée** parce qu'elle servait justement à
empêcher un « correctif » sur un choix volontaire : qui la relit doit voir qu'elle a été levée, et
non ne rien trouver. Composant et décision : `docs/ui-specs/revue.md` § Composants — Volume.

**`docs/archive/TECH_DEBT_AUDIT.md` porte une tâche ouverte (F08) sur un fichier supprimé** —
`frontend/home-sources.ts:40`. Archive, donc non corrigée dans ce geste, mais la case reste cochable
sur un fichier qui n'existe plus.

Étapes 1 à 3 : rien n'est visible pour l'utilisateur avant la fin de la 3. Elles se
livrent ensemble ou l'app reste à moitié dans deux shells.

---

### Sweep 2 — réconcilié au wrap-up du 2026-08-20

**Livré et poussé** (`813b83b` → `f475ae0`, audit du chantier :
`docs/superpowers/changes/2026-08-19-kit-sweep-2/audit.md`, local) :
- Composants restants alignés sur le kit : primaires en aplat d'accent partout (Revue,
  Rekordbox, modale), secondaires en aplat gris sans bordure, pop-up Destination, switch/range/
  tri-état sur l'accent, sélection et survol des catégories Réglages enfin visibles, un seul
  matériau de surface flottante, fonds sémantiques opaques, KEY-LOCK accent/gris, Stop/Écarter
  du Lot en gris à encre danger, bouton armé qui garde l'accent (HIG § Buttons/Role, lu).
- **Journal porté à sa spec** (table, groupes session/jour, segmenté en barre, Annuler au menu
  contextuel + inspecteur) et **l'état « annulé » traverse l'IPC** (`JournalEntry.undone`,
  `MIN(undone)` par lot, test de contrat destructurant, mutation-check exécuté).
- **Colonne Verdict** en Bibliothèque — mapping mesuré contre `worker.rs::verdict_str` ; le
  tableau § 16 ci-dessus corrigé en conséquence (`DUPLICATE` inatteignable, `AUTHENTIQUE` et
  `À VÉRIFIER` ajoutés). Une colonne nouvelle s'insère à son index par défaut dans une
  disposition mémorisée d'avant elle.
- Motion resserrée sur transform/opacity (4 exemptions nommées), focus rings unifiés,
  14 stories neuves (Journal, Verdict), sommaire de `design-system-states.md` renuméroté
  (17/55 offsets étaient déjà faux avant la session).

**Reste, dans l'ordre proposé :**
1. **Passe thème clair des 7 écrans** — tout le sweep s'est mesuré en sombre ; une seule
   capture claire prise (Journal). C'est la moitié non vérifiée du travail livré.
2. O‑2 (§ 11) — encre chaude (H≈77,5) sur fonds froids (H≈286), choix esthétique en attente.
3. `--color-accent-fill` n'est pas value-invariant entre thèmes (suit `--color-hue-blue-solid`,
   L 60,28 clair ↔ 62,43 sombre) — figer ou documenter l'intention.
4. Stories du sweep 1 absentes (boutons accent, chips, segmented, sliders, toast) — le miroir
   Storybook ne couvre que Journal/Verdict et l'existant antérieur.
5. Non observé en conditions réelles, à voir au premier usage : état « Annulé » rendu depuis la
   donnée (aucune entrée annulée en base au 2026-08-20), actions de masse exécutées contre la
   base, Rekordbox avec candidats en attente, segment d'occupation focalisé pendant `dim`.
6. Spectrogramme sans cadre en thème clair — question ouverte de la map #6, inchangée.

Résolu pendant le wrap-up : deux sessions locales avaient été lancées depuis des chips
périmés (les deux sujets étaient déjà traités par `f475ae0`). La session « famille swatch »
a convergé proprement — `57f64b2` renomme les cinq teintes en `.sift-rail-src-dot-*` d'après
leur porteur réel et met `rail-sources.ts` à jour ; son merge `7611875` réconcilie les deux
nettoyages parallèles sans perte. Plus rien à surveiller sur ce point.

**Addendum du 2026-08-20 — le point ci-dessus cachait une perte plus ancienne.** L'auto-assignation
des teintes par ordre d'ajout (`resolveSourceColorKey`, livrée par `4befc09`) n'avait pas survécu à
la fusion 1 : `ab64074` a réécrit le rendu avec un repli **neutre** sur `color_key` null, et
l'inventaire des survivants de `6d1cc85` ne l'a pas comptée — toutes les pastilles étaient grises,
les cinq classes de teinte inatteignables hors IPC manuel, contre § 4 (« couleur de dossier
source ») et § 15 (« leur pastille de couleur »). Restaurée dans `frontend/source-color.ts` (module
pur, testé env Node : cycle gelé, position par id croissant, miroir CSS épinglé). L'override manuel
a **retrouvé son UI le 2026-08-20** (`67cdffb`) : rangée de pastilles dans le menu contextuel des
sources — patron Finder Tags, wireframe validé (variante A, 5 teintes ; sous-menu, sélecteur libre
et rangée de 9 écartés, motifs et citations dans `rail.md` § Interactions) — plus « Couleur
automatique » (`set_source_color(id, null)`, désactivée sans override, jamais retirée). Catalogue
et story : section « Menu contextuel » de `design-system-states.md`, `context-menu.stories.ts`.
Vérifié dans la vraie fenêtre par CDP : cycle pose/retrait contre la base réelle, anneau sur la
teinte résolue, `:hover` moteur (bordure secondaire ; fond et `filter` du `button:hover` générique
battus, mesuré transparent/none), et les **deux thèmes** aux tokens exacts.
Gap antérieur relevé au passage : les états de la pastille de source (teinte / `--error`) n'ont ni
entrée dans `design-system-states.md` ni story — même famille que l'item 4 du « Reste » ci-dessus.
**Clos le 2026-08-20** : entrée « Ligne de source du rail » au catalogue + `rail-sources.stories.ts`
(teintes · `--error` · suspendue), livrés avec l'état « Surveillance suspendue » de `rail.md`
§ États — pastille vidée en contour, teinte conservée, markup extrait en module pur
`rail-source-entry.ts` (précédence échec > suspension gelée par `test/rail-source-entry.test.ts`).

### Passe /simplify du 2026-08-20 — réconciliée au wrap-up suivant

**Soldé** (`e0598ca` + merge `0f17418`, quatre angles de review sur `f1517de..HEAD`) : `rails.ts`
devient le miroir frontend unique de `tags::rail_from_ext` — et corrige un défaut réel, `aif`
manquait de la table de la colonne Verdict ; `fake` passe au rouge dans la file de Revue (le
« vert/ambre uniquement » datait du brief 2026-07, § 16 prime) ; `seg-thumb.ts` fond les six
copies du pouce segmenté (le replay du thème d'`eed4b26` tourne dessus) ; le bouton du Lot quitte
ses couleurs inline pour `.sift-baction--primary/--quiet` à `:hover` réaffirmé, son drain passe à
l'ambre ; `journal.ts` perd sa seconde source de vérité « annulé », ses agrégats dupliqués et ses
scans linéaires, et ses helpers de rendu deviennent purs et exportés — la story les exécute au
lieu d'en recopier 66 lignes.

**Entré, dans l'ordre d'utilité :**
1. `row-selection.ts` partagé — la couche 2 (§ 9) vit en trois copies (bibliothèque, journal,
   file) : math d'index pure, testable env Node, à extraire **au prochain passage** sur l'un des
   trois (verdict de la review : réel mais pas urgent, les copies sont justes aujourd'hui).
2. Registre clavier partagé dans `shortcuts.ts` (`registerListKeyboard`) — deux écouteurs globaux
   non coordonnés se disputeront la couche 2 au premier troisième écran.
3. `errorStateHtml` dans `empty-state.ts` — sept panneaux « message + Réessayer » ad hoc.
4. `basename` frontend ×13, `rowsHtml` ×3 — copies locales, `dom.ts` est la place.
5. `CREATE INDEX idx_actions_batch` (suivi déjà nommé dans `actions.rs:1051`) — `list_journal`
   lit strictement plus de lignes depuis que les lots annulés sont rendus ; et `LIMIT` compte
   désormais les annulés, donc l'historique remonte moins loin à limite égale (documenté dans le
   commentaire SQL).
6. `jrnlState.collapsed` survit au changement d'écran alors que son commentaire dit l'inverse —
   pré-existant, mesuré pendant la passe.
7. Écartés : la coche « Copié » inline est devenue le toast standard (`copyToClipboard` partagé) —
   changement de comportement assumé, à revoir si le toast gêne.

