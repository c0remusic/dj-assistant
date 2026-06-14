# M6a — Identification Discogs — Design

**Projet :** Sift (`dj-assistant`). Sous-projet de la milestone M6 (l'onglet Bibliothèque =
M6b, spec séparée).

**But :** depuis le pane de revue, un bouton **« Identifier »** récupère les métadonnées d'un
morceau sur Discogs (artiste, titre, label, année, sous-genres, pochette, `release_id`), les
propose sans rien écraser, et — sur acceptation — enrichit le `Canonical` qui pilote le nom de
fichier et les tags au moment du rangement.

**Principe directeur (inchangé sur tout Sift) :** on signale/propose, **l'utilisateur décide**.
Rien n'est écrit dans le fichier avant le Rangement.

---

## Décisions (issues du brainstorm)

1. **Déclenchement :** bouton **à la demande** dans la revue (pas d'appel réseau automatique).
   Conçu pour qu'une future vue batch (M7) puisse boucler dessus et auto-appliquer le top.
2. **Sources :** Discogs d'abord, **derrière un trait `MetadataProvider`** pour brancher
   d'autres sources ensuite (AcoustID/MusicBrainz = suite évidente, réutilise nos empreintes
   Chromaprint). Pas de Beatport (pas d'API méta gratuite).
3. **Choix du résultat :** pré-remplir avec le **meilleur match**, lien **« autres »** qui
   déplie les alternatives si c'est faux.
4. **Genres :** **uniquement les `style` Discogs** (sous-genres : « Deep House »…). Le `genre`
   large Discogs (« Electronic ») est ignoré. **Plusieurs sous-genres** possibles → table dédiée.
5. **Token :** jeton personnel Discogs saisi dans **Réglages** (table `settings`, clé
   `discogs_token`). Jamais en dur.
6. **Pochette :** téléchargée en cache, **embarquée dans le fichier au rangement**.

---

## Architecture & composants

### Backend (Rust, `src-tauri/src/`)

**`metadata/mod.rs`** — abstraction de source.
```rust
pub struct Query { pub artist: String, pub title: String }

// Serialize → envoyé à l'UI ; Deserialize → l'UI renvoie le candidat choisi à apply_identity.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Candidate {
    pub artist: String,
    pub title: String,
    pub label: Option<String>,
    pub year: Option<i64>,
    pub styles: Vec<String>,     // sous-genres (Discogs "style"), ordonnés
    pub country: Option<String>,
    pub format: Option<String>,  // ex. "Vinyl, 12\"" / "File, FLAC"
    pub cover_url: Option<String>,
    pub release_id: String,
    pub source: String,          // "discogs"
}

pub trait MetadataProvider {
    /// Cherche des candidats, meilleur match en premier. Erreur typée (voir Erreurs).
    fn search(&self, q: &Query) -> Result<Vec<Candidate>, ProviderError>;
}

pub enum ProviderError {
    NoToken,                 // → l'UI invite à saisir le token
    RateLimited { retry_after_s: u64 },
    Network(String),
    Parse(String),
}
```

**`metadata/discogs.rs`** — `struct Discogs { token: String }` implémente `MetadataProvider`.
- HTTP via **`ureq`** (client bloquant, rustls — colle au backend synchrone, pas de tokio).
- `search()` : `GET /database/search?artist=&track=&type=release` (en-tête
  `Authorization: Discogs token=…` + `User-Agent: Sift/<version>`), puis pour le top-N
  (N=8) on garde les champs de la réponse de recherche (label, year, country, format, genre,
  style, cover_image, id). On **ne** fait **pas** d'appel détail par candidat (économie de
  requêtes) ; le détail release n'est récupéré que si besoin futur.
- Mapping → `Candidate` : `styles = réponse.style` (vide si absent) ; `genre` large ignoré.
- Ordre : conserve l'ordre de pertinence Discogs (déjà trié), filtre les entrées sans titre.

**`metadata/cover.rs`** — `download_cover(url, release_id) -> Result<PathBuf>` : télécharge
(`ureq`) vers `<appdata>/covers/<release_id>.jpg`. Idempotent (si déjà présent, renvoie le
chemin). Échec → `Err` non bloquant côté appelant.

**`ipc_identify.rs`** (nouveau module IPC, miroir de `ipc_filing.rs`) :
- `identify(path) -> Result<Vec<Candidate>, String>` : lit le token (settings) → si vide,
  renvoie une erreur typée sérialisée `"NO_TOKEN"` ; sinon construit `Query` depuis le
  `Canonical` courant du morceau (reconcile tags+nom existant) → `Discogs::search` → renvoie
  la liste. Erreurs mappées en chaînes stables (`NO_TOKEN` / `RATE_LIMITED:<s>` / `NETWORK` /
  `PARSE`) pour que l'UI affiche le bon message.
- `apply_identity(track_id, candidate) -> Result<Canonical, String>` :
  1. upsert `metadata` (artist/title/label/year/cover_path/discogs_release_id/source) en
     `ON CONFLICT(track_id) DO UPDATE`.
  2. remplace les lignes `track_genres` du morceau par les `styles` du candidat (ordonnées).
  3. télécharge la pochette (best-effort) → `cover_path` + `tracks.has_cover=1` si OK.
  4. renvoie le `Canonical` enrichi (pour mettre à jour aperçu + nom proposé côté UI).

**`tagging.rs`** (étendu) — `write_tags` écrit en plus : `label`, `year`, **sous-genres en
multi-valeur** (Vorbis comment / ID3v2.4 TCON multiples ; sinon chaîne jointe `; `) et
**pochette embarquée** (lofty `Picture`) à partir de `cover_path`. Appelé par le flux de
rangement existant — aucune écriture hors rangement.

### Stockage (DB)

`metadata` (existante, keyée `track_id`) réutilisée pour les champs **mono-valeur**.
**Migration v6** : nouvelle table pour les sous-genres multiples.
```sql
CREATE TABLE track_genres (
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    genre    TEXT NOT NULL,   -- un sous-genre Discogs ("Deep House")
    ord      INTEGER NOT NULL,-- ordre d'affichage / d'écriture
    PRIMARY KEY (track_id, genre)
);
```
La colonne `metadata.genre` n'est plus la source de vérité des genres ; laissée pour
compat (non écrite par M6a) — `track_genres` fait foi.

### Frontend (`frontend/`)

- `filing.ts` : ajoute le bouton **« Identifier »** dans le pane de revue. Au clic →
  `invoke("identify", {path})`. États : chargement, liste (top pré-sélectionné + « autres »),
  vide, erreur (message selon le code). La sélection appelle `invoke("apply_identity", …)` →
  met à jour le `Canonical` local (pochette, sous-genres en puces, nom proposé regénéré).
- Affichage : pochette (depuis `convertFileSrc(cover_path)`), label · année · sous-genres
  (puces). Réutilise les conventions visuelles existantes du pane.
- `Réglages` : champ **token Discogs** (write via `settings`). Lien « obtenir un token »
  (page Discogs) ouvert via `open_url` (déjà restreint http(s)).

---

## Flux de données (un geste)

```
[Revue] clic « Identifier »
   → identify(path)
        token vide ? → erreur NO_TOKEN → UI invite Réglages
        sinon Query(artist,title) → Discogs.search → Vec<Candidate> (top en 1er)
   → UI : pré-remplit top, « autres » déplie le reste
[clic sur un candidat]
   → apply_identity(track_id, candidate)
        upsert metadata + remplace track_genres + télécharge pochette (best-effort)
        → renvoie Canonical enrichi
   → UI : aperçu + nom proposé mis à jour ; RIEN écrit dans le fichier
[Rangement] (flux existant inchangé)
   → write_tags étendu : artiste/titre/label/année/sous-genres(multi)/pochette embarquée
   → nom regénéré par le template s'applique au déplacement
```

## Gestion d'erreurs (toutes non bloquantes)

| Cas | Comportement |
|-----|--------------|
| Pas de token | `identify` renvoie `NO_TOKEN` → toast + invite à saisir dans Réglages |
| Réseau KO | `NETWORK` → toast « Discogs injoignable » ; `Canonical` courant conservé |
| Rate-limit 429 | `RATE_LIMITED:<s>` → respecte `Retry-After`, message « réessaie dans Xs » |
| Aucun résultat | liste vide → état « Rien sur Discogs » ; saisie manuelle reste |
| Pochette KO | métadonnées appliquées quand même, pochette ignorée (best-effort) |

## Tests (logique testée, pas le réseau)

- **`metadata::discogs`** : normalisation depuis des **fixtures JSON** capturées (réponse
  `/database/search`) → `Vec<Candidate>` attendu ; vérifie : `styles` = `style` Discogs, `genre`
  large ignoré, ordre conservé, entrées sans titre filtrées, champs optionnels absents → `None`.
- **`apply_identity`** (avec un `MetadataProvider` factice ou candidat construit en dur) :
  - upsert `metadata` (relecture des colonnes) ;
  - **plusieurs `styles` → plusieurs lignes `track_genres`** ordonnées ;
  - remplacement (ré-appliquer écrase les anciens genres, pas d'accumulation) ;
  - `Canonical` renvoyé enrichi.
- **`tagging`** : `write_tags` étendu — round-trip lofty d'un fichier de test : relit
  label/année, **sous-genres multiples**, présence de la pochette embarquée.
- **`settings`** : round-trip `discogs_token` (déjà couvert par les tests settings existants).
- La couche HTTP `ureq` (appels réseau réels) **n'est pas** testée en unité ; gardée fine, la
  logique parse/normalise testée via fixtures.

## Hors périmètre M6a (suite)

- Onglet Bibliothèque, mini-lecteur, dashboard, lien release exact → **M6b**.
- Édition fine de la liste de genres (garder/retirer) → éditeur de métadonnées M6b.
- Vue batch + auto-apply du top → M7 (l'archi le permet déjà).
- AcoustID/MusicBrainz (2ᵉ provider par le son) → après M6a, même trait.

## Dépendances nouvelles

- `ureq` (HTTP bloquant, features rustls) — client réseau minuscule.
- Réutilise : `lofty` (tags + pochette), `rusqlite`, `serde_json`, table `settings`/`metadata`.
