# Session 30/06 — état à la coupure

## Code Sift — où on en est
Tout commité jusqu'au re-skin batch + Journal + Trash centralisé + fix perf analyse
(voir audit/PLAN-SIFT.md pour le détail chantier par chantier — C2-bis, C-bis, etc.
sont à jour). RIEN en cours de modification côté code applicatif à la coupure.

Reste ouvert dans le code (pas commencé, juste discuté) :
- Trois pistes UX sur le rail de validation détail (preview Final Name plus visible,
  séparer destination du bouton File, contraste chips format actif/inactif) — issues
  d'un test de la chaîne sift+PRODUCT.md+interface-design, EN ATTENTE de ton choix.
- Tri de la queue en mode détail (idée notée, pas développée).
- D1/D2/D3 du plan (réconciliation doc, échelle typo, split sift-live.ts).

## Outillage — ce qui a changé ce soir (IMPORTANT, lire avant de continuer)
Gros chantier parallèle sur l'outillage Claude Code, fait en partie par moi (Claude
Sonnet, chat) via Desktop Commander, en partie par toi en session Claude Code séparée.
Tout est documenté, mais NOUVEAU et pas encore éprouvé à l'usage :

- **`docs/skills-registre.md`** (Sift) et son équivalent Tuple : recensement quasi
  exhaustif (~120 outils) de toutes les skills/plugins/agents disponibles, avec verdict
  de pertinence vérifié (pas deviné) pour chacun. À CONSULTER avant d'invoquer un outil
  design en particulier — plusieurs skills au nom rassurant se sont révélées hors-scope
  (design-taste-frontend, ui-ux-pro-max en grande partie, image-to-code, etc.).
- **`CLAUDE.md`** (Sift, Tuple, et global `~/.claude/CLAUDE.md`) : règle de routage
  IMPÉRATIVE ajoutée — avant toute tâche non-triviale, identifier le domaine, consulter
  le registre, invoquer explicitement. Pas encore vu cette règle tenir sur une vraie
  session de travail complète — à observer.
- **Pont Google Stitch** installé et configuré : MCP `stitch` (~/.claude.json,
  authentifié gcloud) + 14 skills (stitch-generate-design, enhance-prompt, etc.).
  Permet d'explorer des directions visuelles pour Sift (direction PAS figée, page
  blanche assumée — "je n'aime pas le design actuel"). JAMAIS TESTÉ EN VRAI ce soir,
  juste installé et documenté. SETUP CONFIRMÉ COMPLET : CLI gcloud installé en local,
  projet GCP créé, facturation activée, auth `init` faite — prêt à l'emploi direct,
  pas de nouvelle étape de configuration nécessaire à la reprise.
- **Chaîne designer Oczkowski** installée (grill-me → design-brief → information-
  architecture → design-tokens → brief-to-tasks → frontend-design → design-review,
  orchestrée par design-flow). Pour un chantier UI complet, pas une retouche.
  ⚠️ COLLISION DE NOM connue et documentée : il existe DEUX skills nommées
  `frontend-design` (le plugin officiel Anthropic, ET celle de la chaîne Oczkowski) —
  non résolu lequel se charge en cas d'invocation ambiguë. Surveiller.
- **`PRODUCT.md`** créé à la racine de Sift (register `product`, accessibilité non
  formelle, anti-référence "pas un jouet", brand "pro sobre + petits détails eye-candy
  discrets"). Débloque `impeccable` qui en avait besoin.
- **`.interface-design/system.md`** existant a une section Mode Batch PÉRIMÉE
  (décrit l'ancien tableau ☑/chip, pas le re-skin réel groupes-repliables) — DETTE
  identifiée, PAS corrigée. À faire dans une session dédiée, lecture du code comme
  preuve.

## Une chose à vérifier en priorité à la reprise
Aucun de ces nouveaux outils (Stitch, chaîne designer, registre) n'a encore servi sur
une vraie tâche. La première fois qu'on les utilise pour de vrai est le test réel —
si quelque chose ne marche pas comme documenté, corriger le registre plutôt que de
contourner en silence (c'est tout l'esprit de ce chantier : pas re-deviner).
