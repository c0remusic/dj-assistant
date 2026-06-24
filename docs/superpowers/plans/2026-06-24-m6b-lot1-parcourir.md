# M6b Lot 1 — Parcourir la bibliothèque (plan d'implémentation)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Câbler l'onglet Bibliothèque de la maquette au réel — lister les morceaux `filed` (recherche + chips qualité + facettes dossier/genre) avec un vrai mini-lecteur.

**Architecture:** Backend = 2 requêtes lecture seule dans `library.rs` exposées via un nouveau `ipc_library.rs` (pattern `State<Mutex<Connection>>` → `Result<T,String>`). Frontend = une fonction `renderBiblioLive()` dans `sift-live.ts` (calquée sur `renderEcartes`) qui remplit le DOM de `app.js`, exposée en `window.__siftBiblio`, appelée par `renderBiblio()`. Actions via le listener délégué existant sur `#pa`.

**Tech Stack:** Rust (rusqlite), Tauri v2 IPC, TypeScript vanilla (Vite), wavesurfer via `report-view.ts`.

Spec : `docs/superpowers/specs/2026-06-24-m6b-library-design.md`.

---

## File Structure

- **Modifier** `src-tauri/src/library.rs` — ajouter `LibraryTrack`, `LibraryFolder`, `LibraryFilter`, `list_filed()`, `folder_facets()`.
- **Créer** `src-tauri/src/ipc_library.rs` — commandes `list_library`, `library_folders`.
- **Modifier** `src-tauri/src/lib.rs` — `mod ipc_library;` + enregistrer les 2 commandes dans `invoke_handler!`.
- **Modifier** `shared/contracts.ts` — interfaces `LibraryTrack`, `LibraryFolder`.
- **Modifier** `frontend/ipc.ts` — wrappers `listLibrary()`, `libraryFolders()`.
- **Modifier** `frontend/sift-live.ts` — `renderBiblioLive()` + expose `window.__siftBiblio` + actions biblio dans le listener `#pa`.
- **Modifier** `frontend/app.js` — appeler `window.__siftBiblio?.()` à la fin de `renderBiblio()`.

Genres : réutiliser `crate::genres::get_genres(conn, id)` (déjà testé) plutôt que deviner le schéma de `track_genres`.

---

## Task 1 : `LibraryTrack` + `list_filed()`

**Files:**
- Modify: `src-tauri/src/library.rs`
- Test: dans `src-tauri/src/library.rs` (`#[cfg(test)] mod tests`)

- [ ] **Step 1 : test qui échoue**

```rust
#[test]
fn list_filed_joins_metadata_and_genres() {
    let conn = crate::db::open_in_memory().expect("db");
    conn.execute(
        "INSERT INTO tracks(id, path, format, bitrate, duration, verdict, status, folder, has_cover)
         VALUES(1, '/lib/House/a.aiff', 'aiff', 1411, 360.0, 'ok', 'filed', 'House', 1)",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO tracks(id, path, format, status) VALUES(2, '/in/pending.mp3', 'mp3', 'pending')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO metadata(track_id, artist, title, label, year, bpm, cover_path, discogs_release_id)
         VALUES(1, 'Mr Fingers', 'Can You Feel It', 'Trax', 1986, 120, '/cache/1.jpg', '12345')",
        [],
    ).unwrap();
    crate::genres::set_genres(&conn, 1, &["House".into(), "Deep House".into()]).unwrap();

    let rows = list_filed(&conn, &LibraryFilter::default()).unwrap();

    assert_eq!(rows.len(), 1, "only filed tracks");
    let t = &rows[0];
    assert_eq!(t.id, 1);
    assert_eq!(t.artist.as_deref(), Some("Mr Fingers"));
    assert_eq!(t.title.as_deref(), Some("Can You Feel It"));
    assert_eq!(t.format.as_deref(), Some("aiff"));
    assert_eq!(t.bitrate, Some(1411));
    assert_eq!(t.verdict.as_deref(), Some("ok"));
    assert_eq!(t.folder.as_deref(), Some("House"));
    assert_eq!(t.discogs_release_id.as_deref(), Some("12345"));
    assert_eq!(t.genres, vec!["House".to_string(), "Deep House".to_string()]);
}
```

> If `crate::db::open_in_memory` doesn't exist, check `db.rs` for the test-DB helper used by other modules (e.g. `open` on a tempfile or a `mem()` test fn) and use that instead — match the existing test pattern in `dedup.rs`/`genres.rs`.

- [ ] **Step 2 : lancer le test → échec attendu**

Run: `cargo test --manifest-path src-tauri/Cargo.toml library::tests::list_filed_joins -- --nocapture`
Expected: FAIL — `cannot find function list_filed` / `LibraryFilter`.

- [ ] **Step 3 : implémentation minimale**

Dans `library.rs` (haut du fichier, après les `use`) :

```rust
use serde::{Deserialize, Serialize};

/// A filed track for the library browser.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryTrack {
    pub id: i64,
    pub path: String,
    pub artist: Option<String>,
    pub title: Option<String>,
    pub format: Option<String>,
    pub bitrate: Option<i64>,
    pub duration: Option<f64>,
    pub bpm: Option<i64>,
    pub year: Option<i64>,
    pub label: Option<String>,
    pub genres: Vec<String>,
    pub discogs_release_id: Option<String>,
    pub cover_path: Option<String>,
    pub has_cover: bool,
    pub verdict: Option<String>,
    pub folder: Option<String>,
}

/// Server-side filters for the library list.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LibraryFilter {
    /// Restrict to one folder (exact match on `tracks.folder`).
    pub folder: Option<String>,
    /// `lossless` (aiff/wav/flac/aif/aiff) or `mp3`; `None`/other = all.
    pub quality: Option<String>,
    /// Restrict by genre (exact, via track_genres).
    pub genre: Option<String>,
    /// Free text over artist/title/path (case-insensitive contains).
    pub q: Option<String>,
}

/// All `filed` tracks joined to their metadata + genres, filtered. Read-only.
pub fn list_filed(conn: &rusqlite::Connection, f: &LibraryFilter) -> rusqlite::Result<Vec<LibraryTrack>> {
    let mut sql = String::from(
        "SELECT t.id, t.path, t.format, t.bitrate, t.duration, t.verdict, t.folder, t.has_cover, \
                m.artist, m.title, m.label, m.year, m.bpm, m.cover_path, m.discogs_release_id \
         FROM tracks t LEFT JOIN metadata m ON m.track_id = t.id \
         WHERE t.status = 'filed'",
    );
    if f.folder.is_some() {
        sql.push_str(" AND t.folder = :folder");
    }
    if let Some(q) = &f.quality {
        match q.as_str() {
            "lossless" => sql.push_str(" AND lower(t.format) IN ('aiff','aif','wav','flac')"),
            "mp3" => sql.push_str(" AND lower(t.format) = 'mp3'"),
            _ => {}
        }
    }
    if f.q.is_some() {
        sql.push_str(" AND (m.artist LIKE :like OR m.title LIKE :like OR t.path LIKE :like)");
    }
    if f.genre.is_some() {
        sql.push_str(" AND t.id IN (SELECT track_id FROM track_genres WHERE genre = :genre)");
    }
    sql.push_str(" ORDER BY m.artist, m.title, t.path");

    let like = f.q.as_ref().map(|q| format!("%{q}%"));
    let mut stmt = conn.prepare(&sql)?;
    let params: Vec<(&str, &dyn rusqlite::ToSql)> = {
        let mut p: Vec<(&str, &dyn rusqlite::ToSql)> = Vec::new();
        if let Some(folder) = &f.folder { p.push((":folder", folder)); }
        if let Some(l) = &like { p.push((":like", l)); }
        if let Some(g) = &f.genre { p.push((":genre", g)); }
        p
    };
    let rows = stmt.query_map(params.as_slice(), |r| {
        Ok((
            r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?,
            r.get::<_, Option<i64>>(3)?, r.get::<_, Option<f64>>(4)?, r.get::<_, Option<String>>(5)?,
            r.get::<_, Option<String>>(6)?, r.get::<_, Option<i64>>(7)?,
            r.get::<_, Option<String>>(8)?, r.get::<_, Option<String>>(9)?,
            r.get::<_, Option<String>>(10)?, r.get::<_, Option<i64>>(11)?,
            r.get::<_, Option<i64>>(12)?, r.get::<_, Option<String>>(13)?,
            r.get::<_, Option<String>>(14)?,
        ))
    })?.collect::<rusqlite::Result<Vec<_>>>()?;

    let mut out = Vec::with_capacity(rows.len());
    for (id, path, format, bitrate, duration, verdict, folder, has_cover, artist, title, label, year, bpm, cover_path, rel) in rows {
        out.push(LibraryTrack {
            id, path, artist, title, format, bitrate, duration, bpm, year, label,
            genres: crate::genres::get_genres(conn, id).unwrap_or_default(),
            discogs_release_id: rel, cover_path,
            has_cover: has_cover.unwrap_or(0) != 0,
            verdict, folder,
        });
    }
    Ok(out)
}
```

> Confirm `genres::set_genres` / `get_genres` signatures in `genres.rs` and the `track_genres` column name `genre` (used in the genre filter subquery). If the column differs, adjust the subquery only.

- [ ] **Step 4 : lancer le test → passe**

Run: `cargo test --manifest-path src-tauri/Cargo.toml library::tests::list_filed_joins`
Expected: PASS.

- [ ] **Step 5 : commit**

```bash
git add src-tauri/src/library.rs
git commit -m "feat(library): list_filed query for the Bibliothèque browser"
```

---

## Task 2 : facettes dossier + genre (`folder_facets()`)

**Files:**
- Modify: `src-tauri/src/library.rs`
- Test: `src-tauri/src/library.rs` tests

- [ ] **Step 1 : test qui échoue**

```rust
#[test]
fn folder_facets_counts_filed_by_folder_and_genre() {
    let conn = crate::db::open_in_memory().expect("db");
    for (id, folder) in [(1, "House"), (2, "House"), (3, "Techno")] {
        conn.execute(
            "INSERT INTO tracks(id, path, status, folder) VALUES(?1, ?2, 'filed', ?3)",
            rusqlite::params![id, format!("/lib/{folder}/{id}.aiff"), folder],
        ).unwrap();
    }
    conn.execute("INSERT INTO tracks(id, path, status, folder) VALUES(9, '/in/p.mp3', 'pending', 'House')", []).unwrap();
    crate::genres::set_genres(&conn, 1, &["House".into()]).unwrap();
    crate::genres::set_genres(&conn, 2, &["House".into()]).unwrap();
    crate::genres::set_genres(&conn, 3, &["Techno".into()]).unwrap();

    let f = folder_facets(&conn).unwrap();

    let house = f.folders.iter().find(|x| x.name == "House").unwrap();
    assert_eq!(house.count, 2, "only filed House tracks");
    assert!(f.folders.iter().find(|x| x.name == "Techno").map(|x| x.count) == Some(1));
    let g_house = f.genres.iter().find(|x| x.name == "House").unwrap();
    assert_eq!(g_house.count, 2);
}
```

- [ ] **Step 2 : lancer → échec**

Run: `cargo test --manifest-path src-tauri/Cargo.toml library::tests::folder_facets_counts`
Expected: FAIL — `cannot find function folder_facets`.

- [ ] **Step 3 : implémentation**

```rust
/// A facet bucket (folder or genre) with its filed-track count.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryFolder {
    pub name: String,
    pub count: i64,
}

/// Both facet lists for the library sidebar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryFacets {
    pub folders: Vec<LibraryFolder>,
    pub genres: Vec<LibraryFolder>,
}

/// Counts of `filed` tracks grouped by folder and by genre. Read-only.
pub fn folder_facets(conn: &rusqlite::Connection) -> rusqlite::Result<LibraryFacets> {
    let mut by = |sql: &str| -> rusqlite::Result<Vec<LibraryFolder>> {
        let mut stmt = conn.prepare(sql)?;
        stmt.query_map([], |r| Ok(LibraryFolder { name: r.get(0)?, count: r.get(1)? }))?
            .collect()
    };
    let folders = by(
        "SELECT folder, COUNT(*) FROM tracks \
         WHERE status='filed' AND folder IS NOT NULL AND folder <> '' \
         GROUP BY folder ORDER BY folder",
    )?;
    let genres = by(
        "SELECT g.genre, COUNT(*) FROM track_genres g \
         JOIN tracks t ON t.id = g.track_id AND t.status='filed' \
         GROUP BY g.genre ORDER BY g.genre",
    )?;
    Ok(LibraryFacets { folders, genres })
}
```

- [ ] **Step 4 : lancer → passe**

Run: `cargo test --manifest-path src-tauri/Cargo.toml library::tests::folder_facets_counts`
Expected: PASS.

- [ ] **Step 5 : commit**

```bash
git add src-tauri/src/library.rs
git commit -m "feat(library): folder+genre facet counts for the browser sidebar"
```

---

## Task 3 : commandes IPC `list_library` + `library_folders`

**Files:**
- Create: `src-tauri/src/ipc_library.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1 : créer `ipc_library.rs`**

```rust
//! IPC surface for the M6b library browser: read-only listing + facets of filed tracks.
use crate::library::{self, LibraryFacets, LibraryFilter, LibraryTrack};
use rusqlite::Connection;
use std::sync::Mutex;
use tauri::State;

/// Filed tracks joined to metadata + genres, filtered (folder / quality / genre / q).
#[tauri::command]
pub fn list_library(
    conn: State<'_, Mutex<Connection>>,
    filter: Option<LibraryFilter>,
) -> Result<Vec<LibraryTrack>, String> {
    let conn = conn.lock().map_err(|e| e.to_string())?;
    library::list_filed(&conn, &filter.unwrap_or_default()).map_err(|e| e.to_string())
}

/// Folder + genre facet counts for the sidebar.
#[tauri::command]
pub fn library_folders(conn: State<'_, Mutex<Connection>>) -> Result<LibraryFacets, String> {
    let conn = conn.lock().map_err(|e| e.to_string())?;
    library::folder_facets(&conn).map_err(|e| e.to_string())
}
```

- [ ] **Step 2 : déclarer le module + enregistrer les commandes dans `lib.rs`**

Ajouter près des autres `mod` (vers le haut de `lib.rs`, à côté de `mod ipc_filing;`):

```rust
mod ipc_library;
```

Dans le `tauri::generate_handler![ … ]`, ajouter (après les `ipc_filing::*`):

```rust
            ipc_library::list_library,
            ipc_library::library_folders,
```

- [ ] **Step 3 : vérifier la compilation + suite verte**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: tout PASS (138+ tests existants + les 2 nouveaux).

Run: `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 4 : commit**

```bash
git add src-tauri/src/ipc_library.rs src-tauri/src/lib.rs
git commit -m "feat(ipc): list_library + library_folders commands"
```

---

## Task 4 : contrats + wrappers IPC frontend

**Files:**
- Modify: `shared/contracts.ts`
- Modify: `frontend/ipc.ts`

- [ ] **Step 1 : ajouter les types dans `shared/contracts.ts`**

```ts
export interface LibraryTrack {
  id: number;
  path: string;
  artist: string | null;
  title: string | null;
  format: string | null;
  bitrate: number | null;
  duration: number | null;
  bpm: number | null;
  year: number | null;
  label: string | null;
  genres: string[];
  discogs_release_id: string | null;
  cover_path: string | null;
  has_cover: boolean;
  verdict: string | null;
  folder: string | null;
}

export interface LibraryFolder { name: string; count: number; }
export interface LibraryFacets { folders: LibraryFolder[]; genres: LibraryFolder[]; }

export interface LibraryFilter {
  folder?: string | null;
  quality?: "lossless" | "mp3" | null;
  genre?: string | null;
  q?: string | null;
}
```

- [ ] **Step 2 : ajouter les wrappers dans `frontend/ipc.ts`**

Importer les types dans le bloc d'import en tête (`LibraryTrack, LibraryFacets, LibraryFilter`), puis en bas du fichier :

```ts
// ---- M6b library browser (mirror of ipc_library.rs) ----

/** Filed tracks for the Bibliothèque list, with optional filters. */
export const listLibrary = (filter?: LibraryFilter): Promise<LibraryTrack[]> =>
  invoke("list_library", { filter: filter ?? null });

/** Folder + genre facet counts for the Bibliothèque sidebar. */
export const libraryFolders = (): Promise<LibraryFacets> =>
  invoke("library_folders");
```

- [ ] **Step 3 : type-check**

Run: `npx tsc --noEmit`
Expected: no errors.

- [ ] **Step 4 : commit**

```bash
git add shared/contracts.ts frontend/ipc.ts
git commit -m "feat(front): library browser IPC contracts + wrappers"
```

---

## Task 5 : `renderBiblioLive()` (liste + recherche + chips + facettes)

**Files:**
- Modify: `frontend/sift-live.ts`
- Modify: `frontend/app.js`

- [ ] **Step 1 : module-level state + import en tête de `sift-live.ts`**

Ajouter aux imports IPC existants : `listLibrary, libraryFolders`. Ajouter les types : `LibraryTrack, LibraryFacets, LibraryFilter`. Puis, près des autres états de module :

```ts
const bibState: { filter: LibraryFilter; facet: "folder" | "genre"; tracks: LibraryTrack[] } = {
  filter: {},
  facet: "folder",
  tracks: [],
};
```

- [ ] **Step 2 : écrire `renderBiblioLive()` (calquée sur `renderEcartes`)**

```ts
function fmtDur(sec: number | null): string {
  if (!sec || sec <= 0) return "—";
  const m = Math.floor(sec / 60), s = Math.round(sec % 60);
  return `${m}:${String(s).padStart(2, "0")}`;
}
function qualPill(t: LibraryTrack): string {
  const f = (t.format || "?").toUpperCase();
  return `<span class="pill" style="flex:none">${esc(f)}</span>`;
}
function verdictBadge(v: string | null): string {
  if (v === "fake") return `<span class="pill" style="background:var(--color-background-danger);color:var(--color-text-danger);flex:none">faux</span>`;
  if (v === "grey") return `<span class="pill" style="background:var(--color-background-warning);color:var(--color-text-warning);flex:none">?</span>`;
  return "";
}

async function renderBiblioLive() {
  const content = document.getElementById("content");
  if (!content) return;
  let facets: LibraryFacets = { folders: [], genres: [] };
  try {
    [bibState.tracks, facets] = await Promise.all([
      listLibrary(bibState.filter),
      libraryFolders(),
    ]);
  } catch (e) {
    console.error("library load failed", e);
    return;
  }

  const chips = (["all", "lossless", "mp3"] as const)
    .map((q) => {
      const on = (bibState.filter.quality ?? "all") === q;
      const label = q === "all" ? "Tous" : q === "lossless" ? "Lossless" : "MP3";
      return `<span class="chip${on ? " on" : ""}" data-bib="qual" data-q="${q}">${label}</span>`;
    })
    .join("");

  const facetList = bibState.facet === "folder" ? facets.folders : facets.genres;
  const sideKey = bibState.facet === "folder" ? "folder" : "genre";
  const activeFacetVal = bibState.facet === "folder" ? bibState.filter.folder : bibState.filter.genre;
  const side =
    `<div style="display:flex;gap:4px;margin-bottom:8px">` +
    `<span class="chip${bibState.facet === "folder" ? " on" : ""}" data-bib="facet" data-f="folder">Dossiers</span>` +
    `<span class="chip${bibState.facet === "genre" ? " on" : ""}" data-bib="facet" data-f="genre">Genres</span></div>` +
    facetList
      .map(
        (b) =>
          `<div class="fld${activeFacetVal === b.name ? " on" : ""}" data-bib="pick" data-key="${sideKey}" data-val="${esc(b.name)}" style="justify-content:space-between"><span>${esc(b.name)}</span><span style="font-size:11px;opacity:.7">${b.count}</span></div>`,
      )
      .join("");

  const rows = bibState.tracks
    .map((t) => {
      const name = esc(t.artist && t.title ? `${t.artist} — ${t.title}` : t.path.split(/[\\/]/).pop() || t.path);
      const link = t.discogs_release_id
        ? `<button class="lk" data-bib="link" data-rid="${esc(t.discogs_release_id)}" aria-label="Fiche Discogs"><i class="ti ti-external-link" style="font-size:13px;color:var(--color-text-tertiary)"></i></button>`
        : `<button class="lk" data-bib="identify" data-id="${t.id}" aria-label="Identifier"><i class="ti ti-search" style="font-size:12px;color:var(--color-text-tertiary)"></i></button>`;
      return `<div class="lr" data-bib="row" data-id="${t.id}"><button class="pb" data-bib="play" data-id="${t.id}" aria-label="Écouter"><i class="ti ti-player-play" style="font-size:12px"></i></button><span style="flex:1;min-width:0;white-space:nowrap;overflow:hidden;text-overflow:ellipsis">${name}</span>${verdictBadge(t.verdict)}${qualPill(t)}<span style="flex:none;width:40px;text-align:right;font-family:var(--font-mono);color:var(--color-text-tertiary)">${fmtDur(t.duration)}</span>${link}</div>`;
    })
    .join("");

  const header =
    `<div style="display:flex;align-items:center;gap:8px;margin-bottom:10px">` +
    `<div style="flex:1;display:flex;align-items:center;gap:7px;border:0.5px solid var(--color-border-secondary);border-radius:var(--border-radius-md);padding:6px 10px"><i class="ti ti-search" style="font-size:14px;color:var(--color-text-tertiary)"></i><input id="bibq" placeholder="Rechercher…" value="${esc(bibState.filter.q || "")}" style="flex:1;border:0;background:transparent;color:inherit;font-size:12px;outline:none"></div>` +
    chips +
    `</div>`;

  content.innerHTML =
    header +
    `<div style="display:flex;gap:14px"><div style="width:150px;flex:none"><div class="col-h">Bibliothèque</div>${side}</div>` +
    `<div style="flex:1;min-width:0"><div style="display:flex;justify-content:space-between;margin-bottom:5px"><span style="font-size:13px;font-weight:500">${esc(activeFacetVal || "Tous")}</span><span style="font-size:11px;color:var(--color-text-tertiary)">${bibState.tracks.length} morceaux</span></div>` +
    (rows || `<div style="font-size:12px;color:var(--color-text-tertiary)">Aucun morceau rangé.</div>`) +
    `<div id="bibplayer"></div></div></div>`;

  const q = document.getElementById("bibq") as HTMLInputElement | null;
  q?.addEventListener("input", () => {
    bibState.filter.q = q.value || undefined;
    clearTimeout((q as unknown as { _t?: number })._t);
    (q as unknown as { _t?: number })._t = window.setTimeout(() => void renderBiblioLive(), 250);
  });
}
```

> `esc()` and the `.chip/.fld/.lr/.pb/.lk/.col-h/.pill` classes already exist (used by `renderEcartes` and `app.js`). Reuse them; do not redefine.

- [ ] **Step 3 : exposer le hook dans `installLiveWiring()`**

Dans `installLiveWiring()` (à côté de `window.__siftEcarts = renderEcartes;`):

```ts
  window.__siftBiblio = () => void renderBiblioLive();
```

Et déclarer le type global (là où `__siftEcarts` etc. sont déclarés — chercher `__siftEcarts` dans le fichier ou un `declare global`):

```ts
    __siftBiblio?: () => void;
```

- [ ] **Step 4 : appeler le hook depuis `app.js`**

Dans `frontend/app.js`, à la **toute fin** de `function renderBiblio(){ … }` (juste avant l'accolade fermante, après le `content.innerHTML=…`), ajouter :

```js
    if(window.__siftBiblio)window.__siftBiblio();
```

- [ ] **Step 5 : vérifier (run)**

Run: `cmd /c "npm run tauri dev"` (ou demander à l'utilisateur de lancer). Aller sur l'onglet Bibliothèque.
Attendu : la liste affiche les vrais morceaux `filed` (ou « Aucun morceau rangé » si la biblio est vide), les facettes Dossiers/Genres avec compteurs, la recherche filtre, les chips Tous/Lossless/MP3 (filtrage câblé au Step suivant). `npx tsc --noEmit` sans erreur.

- [ ] **Step 6 : commit**

```bash
git add frontend/sift-live.ts frontend/app.js
git commit -m "feat(front): wire Bibliothèque list + facets + search to real data"
```

---

## Task 6 : actions biblio (filtre, lien Discogs, mini-lecteur)

**Files:**
- Modify: `frontend/sift-live.ts` (le listener délégué sur `#pa`)

- [ ] **Step 1 : repérer le listener délégué**

Dans `installLiveWiring()`, le listener `document.getElementById("pa")?.addEventListener("click", (e) => { … })` gère déjà les `data-ec` (Écartés). On ajoute la gestion des `data-bib`.

- [ ] **Step 2 : ajouter le handler `data-bib`**

Dans ce listener, après la branche `data-ec`, ajouter :

```ts
    const bibEl = (e.target as HTMLElement).closest<HTMLElement>("[data-bib]");
    if (bibEl) {
      const act = bibEl.dataset.bib;
      if (act === "qual") {
        const q = bibEl.dataset.q;
        bibState.filter.quality = q === "all" ? undefined : (q as "lossless" | "mp3");
        void renderBiblioLive();
      } else if (act === "facet") {
        bibState.facet = bibEl.dataset.f === "genre" ? "genre" : "folder";
        void renderBiblioLive();
      } else if (act === "pick") {
        const key = bibEl.dataset.key as "folder" | "genre";
        const val = bibEl.dataset.val;
        // toggle off if re-clicking the active facet value
        const cur = key === "folder" ? bibState.filter.folder : bibState.filter.genre;
        const next = cur === val ? undefined : val;
        bibState.filter.folder = key === "folder" ? next : undefined;
        bibState.filter.genre = key === "genre" ? next : undefined;
        void renderBiblioLive();
      } else if (act === "link") {
        const rid = bibEl.dataset.rid;
        if (rid) void openUrl(`https://www.discogs.com/release/${rid}`);
      } else if (act === "play") {
        const id = Number(bibEl.dataset.id);
        const t = bibState.tracks.find((x) => x.id === id);
        const host = document.getElementById("bibplayer");
        if (t && host) void openReportInto(host, t.path);
      }
      return;
    }
```

> Import `openUrl` (already in ipc.ts) and `openReportInto` from `./report-view` at the top of `sift-live.ts` if not already imported. `openReportInto(container, path)` mounts the unified detail (waveform + spectro + verdict + player) — this is the "vrai mini-lecteur" and previews the unified-detail direction of Lot 2.

- [ ] **Step 3 : vérifier (run)**

Run: lancer l'app, onglet Bibliothèque :
- cliquer un dossier/genre → la liste se filtre, compteur à jour ;
- chips Tous/Lossless/MP3 → filtre par format ;
- bouton lien → ouvre la page Discogs de la release (vérifier l'URL `release/<id>`) ;
- bouton play d'une ligne → le détail (waveform + spectro + verdict + lecteur) se monte sous la liste et joue.

Probe : morceau sans `discogs_release_id` → bouton « Identifier » (loupe) au lieu du lien ; recherche avec terme sans résultat → « Aucun morceau rangé ».

- [ ] **Step 4 : commit**

```bash
git add frontend/sift-live.ts
git commit -m "feat(front): Bibliothèque filters, Discogs link, inline player"
```

---

## Definition of done (Lot 1)

- `cargo test` + `cargo clippy -D warnings` verts.
- Onglet Bibliothèque : liste réelle des `filed`, facettes Dossiers/Genres avec compteurs, recherche, chips qualité, lien Discogs (ou Identifier), mini-lecteur réel (`report-view`).
- `npx tsc --noEmit` clean.
- Vérifié via `run` (skill verify) sur la vraie app.

Lots suivants (séparés) : 2 édition+ré-identifier, 3 doublons internes, 4 dashboard, 5 audit conformité.
