# Sift — la suite (note de cadrage, pré-brainstorm)

> Note autonome rédigée pendant que tu dors. **Pas une spec** — un point de départ pour qu'on
> fasse le vrai brainstorm interactif → spec → plan au réveil. Je te pose des questions à la fin.

## Où on en est

- **MVP (M0–M5 + M4b)** : surveille les dossiers, analyse (faux/tronqué/clipping), dédup,
  boucle de rangement (encode au format CDJ + nom + tags), Écartés, undo.
- **M6a — Identification Discogs** : ✅ fait + testé en live. Bouton Identifier, recherche `q`,
  nettoyage des artefacts Discogs, préférence release > comp/mix, **match par tracklist
  (titre + version)**, pochette embarquée, sous-genres. Sur la branche `m6a-discogs` (pas
  encore mergé — en attente de ta validation live finale).

## Le « job » du DJ (jobs-to-be-done)

> « Quand j'ai téléchargé un tas de morceaux en vrac, je veux qu'ils soient vérifiés, dédupliqués,
> nommés, taggés, **rangés**, et **sur ma clé prête pour le club** — avec un minimum d'effort. »

Couvert aujourd'hui : vérifier, dédup, nommer, tagger (Discogs). **Manque pour finir le job :**
1. **Parcourir / gérer** sa biblio rangée (voir, ré-écouter, re-ranger, éditer les métadonnées).
2. **Aller jusqu'aux platines** : playlists Rekordbox + clé USB.
3. **Retrouver un morceau même quand le nom est pourri** (le cas qu'on a vécu en testant : nom
   illisible → Discogs ne trouve pas). → identification **par le son**.

## Les candidats pour la suite

### Option A — M6b : onglet Bibliothèque *(complète la boucle M6)*
Parcourir les morceaux rangés : mini-lecteur waveform, re-ranger / re-tagger / supprimer, **lien
vers la release Discogs exacte** (via `release_id` déjà stocké), **édition fine des métadonnées**
(dont la liste de genres — repoussée de M6a), et un **tableau de bord** (% lossless vs MP3,
doublons restants, faux à re-sourcer, par genre).
- *Pour* : rend tout le travail de M6a **visible et gérable** ; transforme Sift de « traiter le
  flux entrant » en « gérer sa collection ». Réutilise des briques existantes (lecteur, actions,
  requêtes). Pas de nouveau service externe.
- *Contre* : ne rapproche pas encore des platines.

### Option B — AcoustID : 2ᵉ source d'identification *par le son* *(suite directe de M6a)*
Brancher AcoustID/MusicBrainz derrière le trait `MetadataProvider` (déjà prévu). **Réutilise les
empreintes Chromaprint qu'on calcule déjà pour la dédup.** Quand le nom est illisible, on
identifie au son.
- *Pour* : règle pile la douleur vécue en test (nom pourri → introuvable). Effort moyen,
  l'archi le permet (trait en place). Pas de token à coller (clé API appli, pas par utilisateur).
- *Contre* : couverture MusicBrainz de l'électro underground < Discogs ; sert surtout de filet
  quand le nom échoue.

### Option C — M7 : export Rekordbox + clé USB *(le vrai payoff club)*
Playlists Rekordbox (XML d'abord, sûr), vue batch/tableau, formatage de clé (FAT32/exFAT).
- *Pour* : **c'est la finalité** — les morceaux arrivent en club.
- *Contre* : plus de valeur **après** que la biblio soit bien rangée/visible (M6b) ; le batch et
  l'écriture Rekordbox sont les morceaux les plus risqués (cf. garde-fous M8).

### Option D — Polish / dette *(transverse, court)*
OAuth Discogs (login au lieu de coller un token — quand on publiera), humaniser les messages
d'erreur, bouton agrandir↔restaurer, retirer les `#![allow(dead_code)]` des modules câblés, +
ce que les audits code/UX en cours remontent.

## Ma recommandation

**M6b (Option A) ensuite**, pour 3 raisons : (1) ça **ferme la boucle M6** et rend les
métadonnées Discogs réellement exploitables ; (2) ça donne le **tableau de bord** qui montre la
valeur de tout le pipeline ; (3) M7 (club) a bien plus de sens une fois la biblio rangée et
visible. **AcoustID (B)** est le meilleur « suivant d'après » — il attaque la douleur du nom
pourri et se greffe proprement sur le trait. M7 (C) en finalité. D (polish) en continu, en
absorbant les findings des audits.

Ordre proposé : **M6b → AcoustID → M7**, polish en continu.

## Questions pour toi (au réveil) — pour lancer le vrai brainstorm

1. D'accord avec **M6b ensuite** ? Ou tu veux **AcoustID** d'abord (résoudre le « nom pourri »
   tout de suite) ?
2. Pour M6b, le **tableau de bord** est-il essentiel dès la v1 de l'onglet, ou d'abord juste
   parcourir + éditer + re-ranger (dashboard après) ?
3. L'**édition des métadonnées** dans la biblio : éditer librement tous les champs (artiste,
   titre, genres, année, label, pochette) + re-tag du fichier ? Ou lecture seule + « ré-identifier
   via Discogs » pour corriger ?
