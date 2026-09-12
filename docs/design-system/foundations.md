# Sift Design System - Foundations

> Couche de lecture du système réel. La source de vérité reste
> `frontend/styles.css` pour les tokens et `docs/design-system-states.md` pour les
> états de composants. Ce dossier documente comment les utiliser, sans créer de
> thème parallèle.

## Produit

Sift est un outil desktop de contrôle qualité et de préparation de musique pour
DJ. Il sert avant le set, pas pendant le live : analyser un fichier, identifier
ses métadonnées, choisir où il doit aller, puis convertir et ranger.

Le principe produit central reste :

> Déplacer = encoder + ranger.

L'écran Revue est donc un poste de décision. Il doit aider l'utilisateur à
répondre vite à quatre questions :

1. Est-ce que le fichier audio est sain pour jouer en club ?
2. Est-ce que l'identification est fiable ?
3. Où dois-je ranger ce morceau ?
4. Sous quel format et sous quel nom final part-il ?

## Utilisateur

L'utilisateur cible est un DJ sérieux qui traite de gros volumes. Il accepte une
interface dense si elle est lisible, stable et honnête. Il n'a pas besoin d'une
interface marketing ou éducative ; il a besoin d'un outil qui lui donne confiance
dans une décision répétitive.

Implications directes :

- densité avant décoration ;
- libellés courts, vocabulaire métier, pas de ton ludique ;
- signal clair entre prêt, douteux, bloqué et actionnable ;
- états persistants sobres, transitions visibles seulement au moment utile ;
- aucune information critique cachée derrière une couleur seule.

## Personnalité Visuelle

Sift doit ressembler à un outil de préparation pro : calme, précis, technique,
mais pas austère. La surface doit rester proche d'un espace de travail continu,
avec des panneaux uniquement quand ils ont une vraie fonction structurelle.

Direction actuelle :

- gris système d'Apple comme base — **changé le 2026-08-16**, la base était « gris chauds »
  depuis l'origine (H≈77-82, chroma ~0,010). Décision d'Antoine sur la ligne 6 de la checklist
  SIGNATURE (issue #26). Ce qui a bougé est la teinte, pas l'échelle : mesurées en OKLCH, les
  valeurs d'Apple tombaient déjà sur les plans de Sift à moins de 1,4 point près, et l'app est
  passée de H≈77 (ambre) à H≈286 (froid), chroma divisée par ~1,5. Valeurs et provenance —
  y compris pourquoi ce sont les gris *iOS* et non macOS — en tête de `frontend/styles.css` ;
- la couleur SÉMANTIQUE, elle, était déjà celle d'Apple depuis 2026-07-06 et n'a pas bougé ;
- vert pour succès/compatibilité ;
- ambre pour attention/décision ;
- rouge pour risque réel ;
- bleu/info uniquement quand l'action ou l'état est informationnel ;
- accents catégoriels réservés aux taxonomies, pas aux titres de section.

Anti-références :

- dashboard SaaS décoratif ;
- landing page ;
- suite audio trop complexe ;
- interface consumer colorée ;
- empilement de cartes sans hiérarchie.

## Sources De Vérité

| Sujet | Source canonique |
|---|---|
| Tokens, couleurs, typo, espacements | `frontend/styles.css` |
| États réels des composants | `docs/design-system-states.md` |
| Vision produit | `PRODUCT.md` |
| Routage skills/agents | `docs/skills/sift-ui-design-governance.md` |
| Règles projet | `AGENTS.md` |
| Maquette exploratoire | `frontend/app.js` — voir la note ci-dessous |

⚠️ `docs/mockups/`, cité ici jusqu'au 2026-08-05, **n'existe pas** : aucun `.html`
nulle part sous `docs/`. La maquette réelle est `frontend/app.js`, chargée
inconditionnellement par `frontend/main.ts` — elle tourne donc en production.

Une maquette peut servir à tester une direction, mais l'app réelle est la
surface de design. Toute décision durable doit finir dans les fichiers de
production et dans la documentation canonique concernée.

## Référence Externe : Apple HIG

Les Apple Human Interface Guidelines sont la référence des macro-décisions desktop
(`CLAUDE.md`). Ce dossier porte la même taxonomie qu'elles — Foundations, Patterns,
Components — et ne les citait pourtant nulle part avant le 2026-08-05.

Accès : les pages HIG sont des SPA. `WebFetch` renvoie une non-réponse qui ressemble à
un refus du modèle ; passer par le Browser pane. `developer.apple.com/design/` est un
hall d'entrée, sans page "principles" : les principes vivent dans les HIG elles-mêmes.

**Test de transposition, à appliquer phrase par phrase.** Sift cible Windows *et* macOS,
avec `decorations: false` (`src-tauri/tauri.conf.json:22`) donc aucune barre de menus
native :

- la règle nomme un **organe du système** — menu bar globale, Dock, Space, position des
  boutons de fenêtre, Dynamic Type, SF Symbols : elle ne vaut que pour la cible macOS ;
- la règle nomme un **fait humain ou matériel** — densité d'information confortable,
  distance de vue 0,3-0,9 m, raccourcis clavier comme accélérateurs, pointage de
  précision, personnalisation des vues : elle vaut pour les deux.

Une convention d'interface n'est jamais juste dans l'absolu : elle est juste parce
qu'elle correspond à une attente déjà installée par le système. Déplacer la convention
sans déplacer l'attente garde la forme et perd la raison.

Résultat mesuré de la confrontation à l'app :
`docs/superpowers/changes/2026-08-05-hig/design.md` — conformités avec leur preuve,
quatre écarts (E1 à E4), trois divergences assumées (D1 à D3), une tension arbitrée.
Ne pas re-dériver ces conclusions de mémoire ; les relire.

Convergence déjà acquise, notée ici parce qu'elle était invisible : `--text-base:13px`
est exactement la taille par défaut macOS des HIG.

## Principe De Travail

Ordre obligatoire :

1. besoin utilisateur ;
2. parcours ;
3. UX ;
4. UI ;
5. performance ;
6. code.

Pour une retouche visuelle, ne pas commencer par "quelle couleur". Commencer par
ce que l'utilisateur doit comprendre ou décider à cet endroit.
