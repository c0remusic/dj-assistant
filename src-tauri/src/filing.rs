//! Turn a reviewed track into a filed library file: ① convert (only if not conformant)
//! → ② tag + name → ③ move into the chosen bin, recording every step as one undoable
//! batch (see actions.rs). Mono-location: conformant files are moved; converted files
//! land in the bin and the original goes to `.sift-trash` (restorable via undo). Composes
//! naming/encode/tagging/library/actions/settings.

use crate::encode::{self, EncodeError, Target};
use crate::naming::{self, Canonical};
use crate::{actions, library, tagging};
use rusqlite::{params, Connection};
use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Sentinel destination meaning "file in place": the track's destination is its OWN source
/// folder, not a bin under the library root. Travels through `bin_rel` like any other
/// destination (the single decision channel) — `plan_file` resolves it instead of `safe_join`.
/// The frontend mirrors this exact literal (`shared/contracts.ts` `FILE_IN_PLACE`); keep them
/// in sync. Must never reach `library::safe_join` (it would create a literal `__SOURCE__` dir).
pub const FILE_IN_PLACE: &str = "__SOURCE__";

/// Prefix marking `bin_rel` as a trusted absolute path OUTSIDE the library root, chosen via a
/// native OS directory picker ("Parcourir un autre dossier…") — the ONE deliberate hole in
/// `safe_join`'s anti-traversal boundary (`library::safe_join` rejects every other absolute or
/// `..`-containing path by design, see its tests). Trust boundary: the frontend must build this
/// value ONLY from a path the Tauri dialog plugin returned (an existing directory the user
/// navigated to and selected), never from free-typed or otherwise user-suppliable text — see
/// `shared/contracts.ts`'s mirror of this exact literal. `plan_file` still re-validates the path
/// actually exists and is a directory before use (defense in depth: the folder could have been
/// deleted or unmounted between the moment it was picked and the moment a file lands there).
pub const EXTERNAL_DEST_PREFIX: &str = "__EXTERNAL__::";

/// Vrai quand `bin_rel` désigne un bac de l'ARBRE de bibliothèque — le SEUL cas où la racine est
/// nécessaire (décision #54, 2026-09-02 : « on doit pouvoir convertir où on veut sans racine »).
///
/// Les deux sentinelles de destination résolvent leur dossier sans racine : `FILE_IN_PLACE` prend
/// le parent de la source, `EXTERNAL_DEST_PREFIX` porte un chemin absolu. La racine n'est donc pas
/// une exigence du rangement, mais de l'arbre — et c'est cette fonction qui porte la distinction,
/// au même endroit que `plan_file` décide `dest_dir`.
pub fn needs_library_root(bin_rel: &str) -> bool {
    bin_rel != FILE_IN_PLACE && !bin_rel.starts_with(EXTERNAL_DEST_PREFIX)
}

/// Why filing could not complete (nothing is left half-filed on these — see ordering).
#[derive(Debug, Clone, PartialEq)]
pub enum FilingError {
    NotFound,
    Upscale,
    /// The source's declared rail (from its extension) diverges from what its content actually
    /// is (lossy content behind a lossless extension — the BUG-1 scenario: e.g. an MP3 renamed
    /// `.flac`). Distinct from `Upscale`: this is a WARN-and-confirm case (FIX-1, option B), not
    /// a hard refusal — the caller can retry `plan_file` with `allow_rail_mismatch=true` once the
    /// user has explicitly confirmed. Stable sentinel string (`"RAIL_MISMATCH"`, mirrors the
    /// existing `"NoLibraryRoot"` convention) so the front can pattern-match it distinctly from
    /// other filing errors.
    RailMismatch,
    /// La destination visée est un bac de l'ARBRE de bibliothèque, et aucune racine n'est réglée.
    /// Chaîne sentinelle stable (`"NoLibraryRoot"`, littéral inchangé depuis `ipc_filing`) : le
    /// front la reconnaît pour router vers Réglages. Depuis #54 elle ne sort plus d'un rangement
    /// EN PLACE ni vers un dossier externe — voir `needs_library_root`.
    NoLibraryRoot,
    /// Une piste DÉJÀ RANGÉE occupe le chemin de destination (`tracks.path` est `UNIQUE`). Distinct
    /// de la ligne `pending` que le watcher vient de créer pour le fichier qu'on est en train de
    /// ranger — celle-là est évincée dans la transaction, sans erreur. Ici c'est un vrai conflit
    /// métier : refus explicite, jamais un `INSERT OR REPLACE` ni un suffixe automatique sur le nom.
    DestOccupied(String),
    Encode(String),
    Tag(String),
    Io(String),
    Db(String),
}

impl std::fmt::Display for FilingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FilingError::NotFound => write!(f, "track not found"),
            FilingError::Upscale => write!(f, "refused: cannot upscale lossy to lossless"),
            FilingError::RailMismatch => write!(f, "RAIL_MISMATCH"),
            FilingError::NoLibraryRoot => write!(f, "NoLibraryRoot"),
            FilingError::DestOccupied(p) => write!(
                f,
                "destination déjà occupée par une autre piste rangée: {p}"
            ),
            FilingError::Encode(m) => write!(f, "encode: {m}"),
            FilingError::Tag(m) => write!(f, "tag: {m}"),
            FilingError::Io(m) => write!(f, "io: {m}"),
            FilingError::Db(m) => write!(f, "db: {m}"),
        }
    }
}

impl From<rusqlite::Error> for FilingError {
    fn from(e: rusqlite::Error) -> Self {
        FilingError::Db(e.to_string())
    }
}

/// Result of filing one track.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FileResult {
    pub path: String,
    pub batch_id: String,
}

/// One track that errored during batch filing (execute or commit failed).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BatchError {
    pub track_id: i64,
    pub message: String,
}

/// Outcome of a batch filing.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BatchResult {
    pub filed: usize,
    pub needs_validation: Vec<i64>,
    /// True when the run was stop-net cancelled before processing every id (the summary is then
    /// partial: what was filed before the stop stays filed — nothing is rolled back).
    pub cancelled: bool,
    pub filed_ids: Vec<i64>,
    pub errors: Vec<BatchError>,
}

/// Outcome of a batch reject (re-sourcing): how many were marked, and which ids failed — so the
/// UI can flag a misfire instead of silently dropping it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RejectBatchResult {
    pub rejected: usize,
    pub failed: Vec<i64>,
}

/// Source path of a track by id. Exposed so an IPC command can resolve the path under the DB
/// lock and then release it before touching the file (see `ipc_filing::reconcile` /
/// `ipc_filing::trash_track`) — the same plan/execute/commit split as `file_track`.
pub fn track_path(conn: &Connection, track_id: i64) -> Result<String, FilingError> {
    conn.query_row(
        "SELECT path FROM tracks WHERE id=?1",
        params![track_id],
        |r| r.get(0),
    )
    .map_err(|_| FilingError::NotFound)
}

/// Lowercased extension (no dot) of a path.
fn ext_of(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// Filename (with extension) component of a path.
fn file_name_of(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .to_string()
}

/// A unique batch id: track id + millis + a process-monotonic counter, so two filings of the
/// same track within the same millisecond (file → undo → re-file) can never share a batch_id.
/// Shared with `apply_tags` so a tag-edit batch gets the same collision-free id scheme.
pub(crate) fn new_batch_id(track_id: i64) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    format!("{track_id}-{ms}-{seq}")
}

/// Reconcile a track's canonical metadata from its embedded tags + filename. Used to pick
/// the green/yellow confidence and to seed the editable fields.
pub fn reconcile_track(conn: &Connection, track_id: i64) -> Result<Canonical, FilingError> {
    let path = track_path(conn, track_id)?;
    Ok(reconcile_path(&path))
}

/// The reconcile of an ALREADY-RESOLVED path: reads the file's embedded tags (disk I/O) and its
/// filename stem. No DB access at all, so a caller that resolved the path under the lock can
/// release it before calling this — see `ipc_filing::reconcile`.
pub fn reconcile_path(path: &str) -> Canonical {
    let (artist, title) = tagging::read_artist_title(path);
    let stem = Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    naming::reconcile(&artist, &title, &stem)
}

/// Centralised trash directory: `{Documents}/Sift/Trash` on all platforms.
/// Falls back to `{home}/Documents/Sift/Trash` if `dirs::document_dir()` returns None.
fn sift_trash_dir() -> Result<PathBuf, FilingError> {
    let base = dirs::document_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join("Documents")))
        .ok_or_else(|| FilingError::Io("cannot locate Documents folder".into()))?;
    Ok(base.join("Sift").join("Trash"))
}

/// Copy `source` to `dest`, verify the copy's size matches, then delete `source`. Cross-disk
/// safe (no rename). On any failure the partial `dest` is cleaned up (best-effort) and `source`
/// is left untouched.
fn copy_verify_delete(source: &str, dest: &Path) -> Result<(), FilingError> {
    let src_len = std::fs::metadata(source)
        .map_err(|e| FilingError::Io(format!("stat source: {e}")))?
        .len();

    std::fs::copy(source, dest).map_err(|e| FilingError::Io(format!("copy: {e}")))?;

    let dst_len = match std::fs::metadata(dest) {
        Ok(m) => m.len(),
        Err(e) => {
            let _ = std::fs::remove_file(dest);
            return Err(FilingError::Io(format!("stat copy: {e}")));
        }
    };

    if dst_len != src_len {
        let _ = std::fs::remove_file(dest);
        return Err(FilingError::Io(format!(
            "copy size mismatch (src {src_len} != dst {dst_len})"
        )));
    }

    std::fs::remove_file(source)
        .map_err(|e| FilingError::Io(format!("remove source after copy: {e}")))
}

/// FIX-10: move `source` to `dest`, trying `rename` first (fast, same-device) and falling back
/// to `copy_verify_delete` only on a genuine cross-device error (Windows os error 17
/// `ERROR_NOT_SAME_DEVICE`, Unix os error 18 `EXDEV`) — a conformant filing or a rollback can
/// cross from the source's disk to the library's (or back), where a plain rename hard-fails.
fn move_cross_disk_safe(source: &str, dest: &Path) -> Result<(), FilingError> {
    match std::fs::rename(source, dest) {
        Ok(()) => Ok(()),
        Err(e) if matches!(e.raw_os_error(), Some(17) | Some(18)) => {
            copy_verify_delete(source, dest)
        }
        Err(e) => Err(FilingError::Io(e.to_string())),
    }
}

/// FS-only: copy `source` into `<Documents>/Sift/Trash/<track_id>__<name>` (collision-free),
/// verify the copy size, then delete the source. Cross-disk safe (no rename) — the trash dir is
/// virtually always on a different disk than the library, so skip straight to copy_verify_delete
/// instead of trying (and failing) rename first every time.
/// Returns the trash path. No DB — journaling is the caller's job.
/// FIX-6: no `root` param — the trash dir is centralized under Documents, never under the
/// library root, so a `root` argument was resolved and threaded through 3 callers for nothing.
///
/// This is the EXECUTE phase of trashing a track (`ipc_filing::trash_track`): the copy is
/// unbounded I/O — a lossless track is tens of megabytes and the trash dir is virtually always
/// on another disk — so it runs with the DB lock released, exactly like `execute_file`'s encode.
/// `commit_trash` then journals the result under the lock.
pub fn trash_file_fs(track_id: i64, source: &str) -> Result<String, FilingError> {
    let trash_dir = sift_trash_dir()?;
    std::fs::create_dir_all(&trash_dir).map_err(|e| FilingError::Io(e.to_string()))?;
    let dest = library::ensure_unique(
        &trash_dir.join(format!("{track_id}__{}", file_name_of(source))),
        None,
    );
    copy_verify_delete(source, &dest)?;
    Ok(dest.to_string_lossy().to_string())
}

/// COMMIT phase of trashing a track (under the DB lock): journal the move as a revertable
/// `trash` action and flip the status. `dest` is what `trash_file_fs` returned, so the file is
/// already moved when this runs — which was ALREADY the ordering inside the pre-split
/// `move_to_trash` (FS first, journal second), so the split adds no new "moved but unjournaled"
/// window beyond the lock re-acquisition itself. Both writes here are fast row updates.
pub fn commit_trash(
    conn: &Connection,
    track_id: i64,
    source: &str,
    dest: &str,
) -> Result<(), FilingError> {
    let batch_id = new_batch_id(track_id);
    actions::record(
        conn,
        &batch_id,
        Some(track_id),
        "trash",
        Some(source),
        Some(dest),
    )
    .map_err(|e| FilingError::Db(e.to_string()))?;
    conn.execute(
        "UPDATE tracks SET status='trash' WHERE id=?1",
        params![track_id],
    )?;
    Ok(())
}

/// Persist canonical metadata for a track (upsert into `metadata`).
fn save_metadata(conn: &Connection, track_id: i64, c: &Canonical) -> Result<(), FilingError> {
    conn.execute(
        "INSERT INTO metadata(track_id, artist, title, version) VALUES(?1,?2,?3,?4)
         ON CONFLICT(track_id) DO UPDATE SET artist=excluded.artist, title=excluded.title, version=excluded.version",
        params![track_id, c.artist, c.title, c.version],
    )?;
    Ok(())
}

/// Enrichment tag fields loaded once (under the lock) so phase 2 writes them without DB access.
#[derive(Default, Clone)]
pub struct TagExtras {
    pub label: Option<String>,
    pub year: Option<i64>,
    pub genres: Vec<String>,
    pub cover_path: Option<String>,
}

/// Load the enrichment tag fields (label, year, genres, cover) for a track from the DB. The single
/// source of these values for tag writes — used by both `plan_file` (filing) and `apply_tags` (the
/// in-place ID3 write), so the two write the SAME label/year/genres/cover a track carries.
pub fn load_tag_extras(conn: &Connection, track_id: i64) -> TagExtras {
    TagExtras {
        label: conn
            .query_row(
                "SELECT label FROM metadata WHERE track_id=?1",
                params![track_id],
                |r| r.get::<_, Option<String>>(0),
            )
            .ok()
            .flatten(),
        year: conn
            .query_row(
                "SELECT year FROM metadata WHERE track_id=?1",
                params![track_id],
                |r| r.get::<_, Option<i64>>(0),
            )
            .ok()
            .flatten(),
        genres: crate::genres::get_genres(conn, track_id).unwrap_or_default(),
        cover_path: conn
            .query_row(
                "SELECT cover_path FROM metadata WHERE track_id=?1",
                params![track_id],
                |r| r.get::<_, Option<String>>(0),
            )
            .ok()
            .flatten(),
    }
}

/// A filing decided under the DB lock (phase 1), ready to run lock-free (phase 2) and then
/// be committed under the lock (phase 3). Holding the connection across the multi-second
/// ffmpeg encode would freeze every other DB user (analysis workers + all IPC); splitting
/// lets the slow encode run lock-free. See `ipc_filing::file_track`.
pub struct FilePlan {
    track_id: i64,
    batch_id: String,
    source: String,
    dest: String,
    conformant: bool,
    target: Target,
    canonical: Canonical,
    bin_rel: String,
    extras: TagExtras,
}

impl FilePlan {
    /// The resolved destination path (as it will land on disk). Exposed so the batch dispatcher can
    /// reserve each planned dest across in-flight plans (phase 2 writes it later) — see
    /// `ipc_filing::run_file_batch`.
    pub fn dest_path(&self) -> &str {
        &self.dest
    }

    /// The journal batch id this filing will be recorded under. Settled at PLAN time (not at
    /// commit), which is what lets the interactive path hand it to the front as its acknowledgement
    /// before phase 2/3 have run — see `ipc_filing::file_track`.
    pub fn batch_id(&self) -> &str {
        &self.batch_id
    }

    /// The track this plan files. Read by the asynchronous interactive path to name the track in
    /// its completion event once the plan itself has been moved onto the background thread.
    pub fn track_id(&self) -> i64 {
        self.track_id
    }
}

/// One filesystem effect performed in phase 2, to be journaled in phase 3. `meta` carries the
/// optional JSON payload of the journal's `meta` column — used by the conformant filing's `tag_edit`
/// row to stash the OLD tags (so a revert can restore them); `None` for the plain move/convert/trash.
pub struct FsLog {
    kind: &'static str,
    from: String,
    to: String,
    meta: Option<String>,
}

/// The string value persisted in `tracks.target_format`.
fn target_str(target: Target) -> &'static str {
    match target {
        Target::Mp3320 => "mp3_320",
        Target::Aiff1644 => "aiff_16_44",
        Target::Wav1644 => "wav_16_44",
    }
}

/// Phase 1 (under the DB lock): resolve metadata + the collision-free destination and apply
/// the no-upscale guard. No slow work — only fast DB reads and a `create_dir_all`.
/// `allow_rail_mismatch`: when false (the default from IPC), a source whose extension claims
/// lossless but whose CONTENT is actually lossy (FIX-1 / BUG-1) is refused with
/// `FilingError::RailMismatch` instead of silently filed — the front shows a confirmation and
/// retries with `true` if the user proceeds anyway.
/// Like `library::ensure_unique`, but also treats every path in `reserved` as taken. In a
/// parallel batch, phase 1 (`plan_file`) is resolved serially under the DB lock, yet the file it
/// names is only WRITTEN in the concurrent phase 2 — so a plain `ensure_unique` (FS-existence
/// only) could hand the SAME destination to two tracks that reconcile to the same name (e.g. two
/// copies of one song filed into one bin), because neither file exists on disk yet when the
/// second plan resolves. Reserving each planned dest closes that window: the second plan skips
/// past the first's not-yet-written path. `ignore` keeps the conformant "file in place" self-name
/// exemption. Bounded bump identical to `ensure_unique`'s (" (N)" before the extension).
fn ensure_unique_reserved(
    path: &Path,
    ignore: Option<&Path>,
    reserved: &HashSet<String>,
) -> PathBuf {
    let taken = |p: &Path| reserved.contains(&p.to_string_lossy().to_string());
    // First let ensure_unique settle FS collisions; then bump further past any reserved sibling.
    let mut candidate = library::ensure_unique(path, ignore);
    if !taken(&candidate) {
        return candidate;
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = path.extension().and_then(|e| e.to_str());
    for n in 2..10_000 {
        candidate = match ext {
            Some(e) => parent.join(format!("{stem} ({n}).{e}")),
            None => parent.join(format!("{stem} ({n})")),
        };
        // Not on disk AND not reserved by an earlier in-flight plan.
        if (!candidate.exists() || ignore.is_some_and(|ig| ig == candidate)) && !taken(&candidate) {
            return candidate;
        }
    }
    parent.join(format!("{stem} ({}).bak", std::process::id()))
}

#[allow(clippy::too_many_arguments)] // each param is an independent, orthogonal input to the
                                     // plan (DB handle, library context, track identity, user overrides) — bundling them into a
                                     // struct here would just move the same 8 fields one level up without reducing real complexity.
pub fn plan_file(
    conn: &Connection,
    // `None` = aucune racine réglée. Ce n'est un refus QUE si `bin_rel` vise l'arbre : les deux
    // sentinelles de destination (en place, dossier externe) n'ont jamais lu cette valeur, et
    // depuis #54 elles ne l'exigent donc plus. Voir `needs_library_root`.
    root: Option<&Path>,
    template: &str,
    track_id: i64,
    bin_rel: &str,
    override_target: Option<Target>,
    edited: Option<Canonical>,
    allow_rail_mismatch: bool,
    // Destinations already claimed by earlier plans whose files aren't written yet (phase 2 is
    // deferred/concurrent). Non-empty for the interactive path too since P5: `file_track` is
    // detached, so its phase 2 has not run by the time the next plan is computed (see
    // `ipc_filing::InFlightFilings`). See `ensure_unique_reserved`.
    reserved: &HashSet<String>,
) -> Result<FilePlan, FilingError> {
    let source = track_path(conn, track_id)?;
    let canonical = match edited {
        Some(c) => c,
        None => reconcile_track(conn, track_id)?,
    };

    let source_rail = crate::analysis::tags::rail_from_ext(&ext_of(&source));
    // BUG-1 guard: the extension says lossless, but is the CONTENT actually lossy? (an MP3
    // renamed `.flac` would otherwise sail through and get "converted" into a fabricated
    // lossless AIFF/WAV — exactly what guard_no_upscale below exists to block, except it never
    // sees the real codec because `source_rail` here is extension-derived, not content-derived.)
    // Only probe content when it matters (declared lossless) — skip the extra I/O for a
    // declared-lossy source, where a content mismatch only means an unnecessary downscale, not
    // a fabricated-lossless risk.
    if source_rail == crate::analysis::Rail::Lossless
        && !allow_rail_mismatch
        && crate::analysis::tags::rail_from_content(&source) == crate::analysis::Rail::Lossy
    {
        return Err(FilingError::RailMismatch);
    }
    let target = override_target.unwrap_or_else(|| encode::target_for(source_rail));
    if encode::guard_no_upscale(source_rail, target).is_err() {
        return Err(FilingError::Upscale);
    }

    // A conformant file is MOVED as-is (no transcode), so its container is unchanged — keep its own
    // extension instead of forcing target.ext(). This stops a `.aif` source from being renamed to
    // `.aiff`: with a single possible output name, a blocked revert (external lock, os error 32 —
    // proved in the revert-duplicate relevé) can no longer strand a `.aif` beside a `.aiff`. The
    // conversion path produces a genuinely new file, which keeps the canonical target extension.
    let conformant = encode::is_conformant(&source, target);
    let out_ext = if conformant {
        ext_of(&source)
    } else {
        target.ext().to_string()
    };

    // The single point where the destination directory is decided. The `FILE_IN_PLACE` sentinel
    // means "file into the track's own source folder" — resolve it to `source.parent()` and NEVER
    // route it through `safe_join` (which would sanitize it into a literal `root/__SOURCE__` dir).
    let dest_dir = if bin_rel == FILE_IN_PLACE {
        Path::new(&source)
            .parent()
            .ok_or_else(|| FilingError::Io("source file has no parent directory".into()))?
            .to_path_buf()
    } else if let Some(abs) = bin_rel.strip_prefix(EXTERNAL_DEST_PREFIX) {
        // Trusted external folder (see EXTERNAL_DEST_PREFIX doc) — still re-checked here, not
        // taken on faith: fail loudly if it's gone rather than silently recreating it elsewhere
        // (create_dir_all below would otherwise happily conjure a new, wrong directory).
        let p = PathBuf::from(abs);
        // Le doc-comment d'`EXTERNAL_DEST_PREFIX` promet un chemin ABSOLU (issu du dialog natif).
        // Rendu exécutable le 2026-09-02 : depuis #54 cette branche est LE chemin de rangement sans
        // racine, donc la seule chose qui la sépare encore de `safe_join`. Un relatif y résoudrait
        // contre le répertoire courant du process — ni la bibliothèque, ni un dossier que
        // l'utilisateur a désigné. Refus AVANT `is_dir()` : un relatif qui existe par hasard sous
        // le cwd passerait le test d'existence tout en violant le contrat.
        if !p.is_absolute() {
            return Err(FilingError::Io(format!(
                "external destination must be an absolute path: {abs}"
            )));
        }
        // Le doc-comment d'`EXTERNAL_DEST_PREFIX` promet un chemin ABSOLU (issu du dialog natif).
        // Rendu exécutable le 2026-09-02 : depuis #54 cette branche est LE chemin de rangement sans
        // racine, donc la seule chose qui la sépare encore de `safe_join`. Un relatif y résoudrait
        // contre le répertoire courant du process — ni la bibliothèque, ni un dossier que
        // l'utilisateur a désigné. Refus AVANT `is_dir()` : un relatif qui existe par hasard sous
        // le cwd passerait le test d'existence tout en violant le contrat.
        if !p.is_dir() {
            return Err(FilingError::Io(format!(
                "external destination no longer exists: {abs}"
            )));
        }
        p
    } else {
        // Seul point du plan qui LIT la racine — donc le seul qui peut la manquer. Le refus naît
        // ici et nulle part ailleurs : c'est ce qui rend la sentinelle proportionnée à la
        // destination visée plutôt qu'au geste de rangement (#54).
        let root = root.ok_or(FilingError::NoLibraryRoot)?;
        library::safe_join(root, bin_rel).map_err(FilingError::Io)?
    };
    std::fs::create_dir_all(&dest_dir).map_err(|e| FilingError::Io(e.to_string()))?;
    let filename = naming::render_filename(template, &canonical, &out_ext);
    // Ignore the source itself as a collision ONLY for the conformant (move) path: filing a
    // conformant track in place onto its own (already-correct) name must keep that name, not bump
    // it to " (2)". The non-conformant path ENCODES source → dest, so dest must never equal source
    // (FFmpeg reading and writing the same file would corrupt it) — keep the normal collision bump.
    let ignore_self = if conformant {
        Some(Path::new(&source))
    } else {
        None
    };
    let dest = ensure_unique_reserved(&dest_dir.join(&filename), ignore_self, reserved);

    let extras = load_tag_extras(conn, track_id);

    Ok(FilePlan {
        conformant,
        source,
        dest: dest.to_string_lossy().to_string(),
        target,
        canonical,
        bin_rel: bin_rel.to_string(),
        batch_id: new_batch_id(track_id),
        track_id,
        extras,
    })
}

/// Phase 2 (NO DB lock): the slow work — tag + move, or encode + tag + trash. Leaves the
/// filesystem clean on its own failure (no orphan transcode). Returns the effects to journal.
pub fn execute_file(plan: &FilePlan) -> Result<Vec<FsLog>, FilingError> {
    let mut log = Vec::new();
    if plan.conformant {
        // A conformant filing tags the file IN PLACE then MOVES it — no trashed original to restore
        // from. So capture the OLD tags FIRST (fail clear if unreadable — never file without the net),
        // and journal them as a `tag_edit` row BEFORE the `move`. revert_batch undoes newest-first, so
        // it reverses the move (file back at `source`) THEN restores the old tags at `source` — the
        // exact path the tag_edit row points at. Reuses the B4 snapshot/restore mechanism verbatim.
        let old_tags = tagging::read_tags_full(&plan.source).map_err(FilingError::Tag)?;
        let snapshot = serde_json::to_string(&old_tags)
            .map_err(|e| FilingError::Tag(format!("serialize tag snapshot: {e}")))?;
        log.push(FsLog {
            kind: "tag_edit",
            from: plan.source.clone(),
            to: plan.source.clone(),
            meta: Some(snapshot),
        });
        tagging::write_tags_full(
            &plan.source,
            &plan.canonical.artist,
            &naming::tag_title(&plan.canonical),
            plan.extras.label.as_deref(),
            plan.extras.year,
            &plan.extras.genres,
            plan.extras.cover_path.as_deref(),
        )
        .map_err(FilingError::Tag)?;
        // Le `?` nu manquait ici, et c'était la seule fenêtre du chemin conformant où les tags
        // étaient DÉJÀ écrasés sur le fichier de l'utilisateur sans que rien ne puisse les
        // remettre. Le déplacement échoue (disque plein, destination verrouillée, permission) et
        // la fonction sortait en laissant le fichier a sa place SOURCE, avec les nouveaux tags
        // écrits en place — et sans ligne de journal, puisque le journal n'est écrit qu'en phase 3
        // depuis le `log` RETOURNE. Donc: aucun revert possible depuis l'app, aucune trace, et des
        // tags que l'utilisateur n'a pas demandés sur un fichier qu'il croit intact.
        //
        // `log` porte déjà la ligne `tag_edit` avec l'instantané des anciens tags (poussée juste
        // au-dessus, AVANT l'écriture, précisément pour ce cas). `rollback_fs` sait la rejouer.
        // C'est le même filet que celui de la phase 3 (commit_file), appliqué à la seule étape qui
        // en était privée. Audit 2026-07-28, CR-3.
        if let Err(e) = move_cross_disk_safe(&plan.source, Path::new(&plan.dest)) {
            log::error!(
                "execute_file: move a échoué pour {}, restauration des tags d'origine: {e:?}",
                plan.source
            );
            rollback_fs(&log);
            return Err(e);
        }
        log.push(FsLog {
            kind: "move",
            from: plan.source.clone(),
            to: plan.dest.clone(),
            meta: None,
        });
    } else {
        // transcode into the bin, tag the result, then trash the original (mono-location)
        encode::encode(&plan.source, &plan.dest, plan.target).map_err(|e| match e {
            EncodeError::Upscale => FilingError::Upscale,
            EncodeError::Ffmpeg(m) => FilingError::Encode(m),
        })?;
        if let Err(e) = tagging::write_tags_full(
            &plan.dest,
            &plan.canonical.artist,
            &naming::tag_title(&plan.canonical),
            plan.extras.label.as_deref(),
            plan.extras.year,
            &plan.extras.genres,
            plan.extras.cover_path.as_deref(),
        ) {
            let _ = std::fs::remove_file(&plan.dest); // drop the orphan transcode
            return Err(FilingError::Tag(e));
        }
        log.push(FsLog {
            kind: "convert",
            from: plan.source.clone(),
            to: plan.dest.clone(),
            meta: None,
        });
        match trash_file_fs(plan.track_id, &plan.source) {
            Ok(trash) => log.push(FsLog {
                kind: "trash",
                from: plan.source.clone(),
                to: trash,
                meta: None,
            }),
            Err(e) => {
                let _ = std::fs::remove_file(&plan.dest);
                return Err(e);
            }
        }
    }
    Ok(log)
}

/// Reverse phase-2 filesystem effects (newest first) — used when phase 3 cannot commit.
///
/// Chaque étape reste best-effort (on continue le déroulé même si l'une échoue : abandonner à
/// mi-chemin laisserait un état plus abîmé que d'essayer les suivantes), mais AUCUNE n'est
/// silencieuse. C'est le dernier filet du seul chemin où un fichier de l'utilisateur peut se
/// retrouver ailleurs que là où il l'a laissé : sans trace, un rollback partiel est
/// indiagnosticable après coup — le fichier a « disparu » et rien dans le journal ne dit où.
fn rollback_fs(log: &[FsLog]) {
    for fs in log.iter().rev() {
        match fs.kind {
            // FIX-10: same cross-disk-safe fallback as the forward move — a rollback of the
            // conformant path's rename can cross disks too.
            "move" | "trash" => {
                if let Err(e) = move_cross_disk_safe(&fs.to, Path::new(&fs.from)) {
                    log::error!(
                        "rollback_fs: remise en place impossible ({}), le fichier reste en {} au lieu de {}: {e:?}",
                        fs.kind,
                        fs.to,
                        fs.from
                    );
                }
            }
            "convert" => {
                if let Err(e) = std::fs::remove_file(&fs.to) {
                    log::error!(
                        "rollback_fs: suppression du fichier converti {} impossible, il reste sur le disque: {e}",
                        fs.to
                    );
                }
            }
            // Conformant filing: undo the in-place tag write by restoring the captured old tags at
            // `from` (the file is back there — the move row, newer, was reversed just above). Reuses
            // the B4 restore.
            "tag_edit" => {
                match &fs.meta {
                    Some(meta) => match serde_json::from_str::<tagging::TagsSnapshot>(meta) {
                        Ok(snap) => {
                            if let Err(e) = tagging::restore_tags(&fs.from, &snap) {
                                log::error!(
                                    "rollback_fs: restauration des tags d'origine de {} impossible, le fichier garde les tags du rangement avorte: {e:?}",
                                    fs.from
                                );
                            }
                        }
                        Err(e) => log::error!(
                            "rollback_fs: snapshot de tags illisible pour {}, tags d'origine perdus: {e}",
                            fs.from
                        ),
                    },
                    // Une ligne `tag_edit` sans meta ne peut pas être défaite : le snapshot EST la
                    // seule copie des anciens tags.
                    None => log::error!(
                        "rollback_fs: ligne tag_edit sans snapshot pour {}, tags d'origine perdus",
                        fs.from
                    ),
                }
            }
            other => log::error!(
                "rollback_fs: type d'effet inconnu {other:?} pour {}, non défait",
                fs.from
            ),
        }
    }
}

/// Phase 3 (under the DB lock): journal the effects + mark the track filed. On any DB error,
/// reverse the filesystem effects so nothing is left half-filed.
///
/// All DB writes for the track (its journal rows + the `tracks` UPDATE + the `metadata` upsert)
/// run in ONE SQLite transaction (`unchecked_transaction`, since we only hold `&Connection`)
/// instead of 4-5 implicit auto-commits per track — one WAL fsync per track instead of several.
/// A DB error rolls the whole transaction back (no partial journal) AND reverses the filesystem
/// effects, then returns `Db` — the fail-fast contract callers already handle per track.
///
/// The linked-Rekordbox-XML repair (read+parse+rewrite of an external file — disk I/O) is
/// DEFERRED until AFTER the transaction commits, via `actions::maybe_repair_rekordbox_xml`, so
/// the slow file I/O never runs inside the write transaction. Its behaviour is unchanged
/// (`move`/`convert` rows only, `from != to`) — only its timing moves from mid-insert to
/// post-commit. It cannot fail the filing (errors are logged and swallowed, as before).
///
/// `xml_repair_sink`: when `Some`, move/convert `(from, to)` pairs needing an XML repair are
/// pushed here INSTEAD OF being repaired immediately — the caller (`ipc_filing::run_file_batch`)
/// collects them across every track in a batch and repairs the linked XML ONCE via
/// `actions::repair_rekordbox_xml_batch`, instead of once per track (audited 2026-07-05, finding
/// P4: up to 200 independent read+parse+write cycles of the same file on a 200-track batch). `None`
/// preserves the original immediate-repair behaviour, used by the single-file commit path
/// (`ipc_filing::file_track`) where there is only ever one pair, so batching buys nothing.
/// `masterdb_index`: l'index `master.db` déchiffré, ou `None` quand rien n'est lié. Fourni par
/// l'appelant pour la MÊME raison que `xml_repair_sink` : `commit_file` tourne sous le verrou
/// global et une fois PAR PISTE, alors que le déchiffrement d'un `master.db` multi-Mo ne dépend
/// pas de la piste. Le résoudre ici revenait à déchiffrer le même fichier 200 fois, verrou tenu,
/// sur un lot de 200 pistes. L'appelant le résout une fois, verrou relâché
/// (`actions::masterdb_path_if_linked` puis `actions::read_masterdb_index`).
pub fn commit_file(
    conn: &Connection,
    plan: &FilePlan,
    log: Vec<FsLog>,
    xml_repair_sink: Option<&mut Vec<(String, String)>>,
    masterdb_index: Option<&crate::rekordbox_masterdb::RekordboxIndex>,
) -> Result<FileResult, FilingError> {
    let conf = match plan.canonical.confidence {
        naming::Confidence::Green => "green",
        naming::Confidence::Yellow => "yellow",
    };

    // Le fichier tel qu'il est MAINTENANT sur le disque : phase 2 est faite, `plan.dest` existe et
    // porte la taille/date d'APRÈS l'écriture des tags et le déplacement. Lu ICI (hors transaction,
    // avant de prendre le verrou d'écriture SQLite) parce que c'est de l'I/O.
    //
    // Échec de lecture = les deux colonnes restent inchangées, le rangement PASSE quand même : un
    // `stat` qui échoue sur un fichier qu'on vient d'écrire est une anomalie de l'environnement,
    // pas une raison de défaire un déplacement réussi. Conséquence assumée et journalisée : un
    // rescan du watcher pourra repasser cette piste en `pending` (voir condition (b) ci-dessous).
    let dest_meta = match std::fs::metadata(&plan.dest) {
        Ok(m) => Some((m.len() as i64, crate::scanner::mtime_secs(&m))),
        Err(e) => {
            log::error!(
                "commit_file: metadata({}) illisible ({e}): size_bytes/mtime laissés inchangés, un rescan du watcher pourra repasser la piste {} en pending",
                plan.dest,
                plan.track_id
            );
            None
        }
    };
    let dest_size = dest_meta.map(|(size, _)| size);
    let dest_mtime = dest_meta.map(|(_, mtime)| mtime);
    let dest_name = file_name_of(&plan.dest);

    // One transaction for every DB write of this track. Dropping it without `commit()` (the `?`
    // early-returns below) rolls back all inserts/updates automatically — no manual DELETE needed.
    let db_result: Result<Vec<i64>, FilingError> = (|| {
        let tx = conn.unchecked_transaction()?;
        let mut action_ids = Vec::with_capacity(log.len());
        for fs in &log {
            let id = actions::record_row_only(
                &tx,
                &plan.batch_id,
                Some(plan.track_id),
                fs.kind,
                Some(&fs.from),
                Some(&fs.to),
                fs.meta.as_deref(),
            )?;
            action_ids.push(id);
        }
        // INVARIANT — `path` suit le fichier. Le rangement déplace ET renomme (`execute_file`) :
        // une piste dont `tracks.path` reste sur la source pointe un fichier inexistant, et la
        // Bibliothèque — qui lit ce champ via `library::list_filed` puis `library-detail.ts` →
        // `openReportInto(track.path)` — ne peut plus la lire (constaté sur un cas réel le
        // 2026-07-30 ; `scanner::forget_path` ne rattrape rien, il ne supprime que les `pending`).
        //
        // Écrire `path=?` SEUL était pire que le bug : essayé puis retiré le 2026-07-31, le
        // crosscheck de la gate ayant montré deux régressions. Les trois conditions ci-dessous sont
        // INDISSOCIABLES — chacune n'existe que parce que `path` bouge, et deux d'entre elles vivent
        // hors de cette fonction. Elles sont couvertes par quatre tests nommés dans ce fichier ;
        // toucher l'une sans les autres rouvre le défaut qu'elle couvre.
        //
        // (a) `actions::revert_batch` doit REMETTRE `path` (+ `size_bytes`/`mtime`/`filename`) sur
        //     la source, sinon une piste annulée garde le chemin d'un fichier que `revert_one_fs`
        //     vient de rendre à sa source : le même bug en miroir.
        //     → `actions.rs`, test `revert_batch_ramene_le_chemin_et_les_metadonnees_sur_la_source`.
        // (b) `size_bytes`/`mtime`/`filename` doivent suivre le déplacement, ici. Un rangement EN
        //     PLACE (`FILE_IN_PLACE`) résout `dest_dir` vers `source.parent()`, donc un dossier
        //     SURVEILLÉ : au passage suivant, `scanner::upsert_file` trouverait à ce chemin une
        //     ligne portant la taille/date d'AVANT l'écriture des tags, prendrait la branche
        //     `Some(_)` (`scanner.rs:88`) et repasserait la piste en `pending` en effaçant
        //     `analyzed_at`/`fingerprint`/`report_json` — la piste rangée se dérangerait seule.
        //     → test `un_rescan_du_fichier_range_en_place_ne_le_derange_pas`.
        // (c) `tracks.path` est `NOT NULL UNIQUE` (`db.rs:21`). Entre le déplacement et cette
        //     transaction, le watcher a pu INSÉRER une ligne `pending` pour le fichier à sa
        //     destination ; sans le DELETE ci-dessous l'`UPDATE` violerait la contrainte,
        //     `rollback_fs` défairait le déplacement et le rangement échouerait pour l'utilisateur.
        //     Le DELETE est volontairement étroit — `status='pending'` ET `id<>` la nôtre : c'est
        //     exactement la ligne parasite que le watcher vient de créer pour le fichier que nous
        //     sommes en train de ranger, jamais une piste déjà rangée. Ce qui survit au DELETE est
        //     un vrai conflit métier → `DestOccupied`, pas de `INSERT OR REPLACE`, pas de suffixe
        //     automatique sur le nom.
        //     → tests `commit_file_evince_la_ligne_pending_concurrente_a_la_destination` et
        //       `commit_file_refuse_une_destination_occupee_par_une_piste_rangee`.
        tx.execute(
            "DELETE FROM tracks WHERE path=?1 AND id<>?2 AND status='pending'",
            params![plan.dest, plan.track_id],
        )?;
        let occupied: i64 = tx.query_row(
            "SELECT count(*) FROM tracks WHERE path=?1 AND id<>?2",
            params![plan.dest, plan.track_id],
            |r| r.get(0),
        )?;
        if occupied > 0 {
            return Err(FilingError::DestOccupied(plan.dest.clone()));
        }
        tx.execute(
            "UPDATE tracks SET status='filed', folder=?2, target_format=?3, confidence=?4,
                    path=?5, filename=?6,
                    size_bytes=COALESCE(?7, size_bytes), mtime=COALESCE(?8, mtime)
             WHERE id=?1",
            params![
                plan.track_id,
                plan.bin_rel,
                target_str(plan.target),
                conf,
                plan.dest,
                dest_name,
                dest_size,
                dest_mtime
            ],
        )?;
        save_metadata(&tx, plan.track_id, &plan.canonical)?;
        tx.commit()?;
        Ok(action_ids)
    })();

    let action_ids = match db_result {
        Ok(ids) => ids,
        Err(e) => {
            // Transaction already rolled back the DB rows; reverse the filesystem effects too so
            // nothing is left half-filed. L'erreur est propagée TELLE QUELLE (elle est déjà une
            // `FilingError`) : la ré-emballer en `Db(e.to_string())` écrasait la variante — un
            // `DestOccupied`, seul cas où l'appelant peut dire à l'utilisateur ce qui bloque,
            // ressortait en « db: destination déjà occupée… », indistinguable d'une panne SQLite.
            rollback_fs(&log);
            return Err(e);
        }
    };

    // A track just became 'filed' — invalidate the dashboard duplicate-count cache. The cache key
    // (COUNT, MAX(id) of filed) misses an in-place re-filing that leaves both unchanged, so we
    // invalidate explicitly rather than rely on the key changing (coordination with R1's cache).
    crate::library::invalidate_duplicate_count_cache();

    // Committed — now (and only now) patch a linked Rekordbox XML for the move/convert rows, and
    // detect (read-only) any master.db repair candidates for the same rows (M8 Tier 1 IPC wiring),
    // plus (M8 Tier 3) any metadata sync candidate for the tags this commit just wrote. Both
    // detectors need the same decrypted `master.db` index — d'où le paramètre : une seule lecture
    // pour les deux détecteurs ET pour toutes les pistes du lot, au lieu d'une par commit.
    let mut xml_repair_sink = xml_repair_sink;
    for (fs, action_id) in log.iter().zip(action_ids.iter()) {
        match xml_repair_sink.as_mut() {
            Some(sink) => {
                if matches!(fs.kind, "move" | "convert") && fs.from != fs.to {
                    sink.push((fs.from.clone(), fs.to.clone()));
                }
            }
            None => {
                actions::maybe_repair_rekordbox_xml(conn, fs.kind, Some(&fs.from), Some(&fs.to))
            }
        }
        if let Some(index) = &masterdb_index {
            actions::maybe_detect_masterdb_repair_with_index(
                conn,
                index,
                fs.kind,
                Some(&fs.from),
                Some(&fs.to),
                *action_id,
            );
            if matches!(fs.kind, "move" | "convert") {
                let (genre, label) = actions::sanitize_genre_label(
                    &plan.extras.genres,
                    plan.extras.label.as_deref(),
                );
                let values = actions::MetadataSyncValues {
                    artist: Some(plan.canonical.artist.clone()),
                    title: Some(naming::tag_title(&plan.canonical)),
                    label,
                    year: plan.extras.year,
                    genre,
                };
                actions::detect_masterdb_metadata_sync_with_index(
                    conn,
                    index,
                    &fs.from,
                    plan.track_id,
                    &values,
                    *action_id,
                );
                if let Some(cover_path) = &plan.extras.cover_path {
                    actions::detect_masterdb_artwork_sync_with_index(
                        conn,
                        index,
                        &fs.from,
                        plan.track_id,
                        cover_path,
                        *action_id,
                    );
                }
            }
        }
    }
    Ok(FileResult {
        path: plan.dest.clone(),
        batch_id: plan.batch_id.clone(),
    })
}

/// File one track into `bin_rel` under `root`, holding `conn` throughout — a synchronous test
/// convenience that chains the three phases under a single lock. Production never holds the lock
/// across the encode: the interactive path (`ipc_filing::file_track`) and the detached batch
/// (`ipc_filing::run_file_batch`) run the phases with the lock released around it. See module docs
/// for the ordering and the mono-location / undo contract.
#[cfg(test)]
#[allow(clippy::too_many_arguments)] // mirrors plan_file's shape (test-only convenience wrapper)
pub fn file_track(
    conn: &Connection,
    root: &Path,
    template: &str,
    track_id: i64,
    bin_rel: &str,
    override_target: Option<Target>,
    edited: Option<Canonical>,
    allow_rail_mismatch: bool,
) -> Result<FileResult, FilingError> {
    let plan = plan_file(
        conn,
        Some(root),
        template,
        track_id,
        bin_rel,
        override_target,
        edited,
        allow_rail_mismatch,
        &HashSet::new(),
    )?;
    let log = execute_file(&plan)?;
    commit_file(
        conn,
        &plan,
        log,
        None,
        actions::resolve_masterdb_index_if_linked(conn).as_ref(),
    )
}

/// Canonical metadata persisted by an earlier Discogs identification (the `metadata` table),
/// if present and usable. A Discogs/manual match is a high-confidence name, so it's returned
/// Green — this is what lets a per-track identity applied in Review feed `file_batch` (whose
/// tag-based reconcile would otherwise ignore the applied identity). `None` = no usable row,
/// fall back to reconcile.
fn canonical_from_metadata(
    conn: &Connection,
    track_id: i64,
) -> rusqlite::Result<Option<Canonical>> {
    let row = conn.query_row(
        "SELECT artist, title FROM metadata WHERE track_id=?1",
        params![track_id],
        |r| {
            Ok((
                r.get::<_, Option<String>>(0)?,
                r.get::<_, Option<String>>(1)?,
            ))
        },
    );
    match row {
        Ok((Some(a), Some(t))) if !a.trim().is_empty() && !t.trim().is_empty() => {
            Ok(Some(Canonical {
                artist: a,
                title: t,
                version: None,
                label: None,
                confidence: naming::Confidence::Green,
            }))
        }
        Ok(_) => Ok(None),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Pick the canonical name to AUTO-file a batch track on, or `None` if it must stay pending for
/// manual review. A track identified via Discogs (persisted in `metadata`) files on that
/// high-confidence name; otherwise the tag/filename reconcile must come out Green. Pure DB read —
/// the detached batch loop (`ipc_filing::file_batch`) calls this under the per-file lock, before
/// planning the file, then runs the same plan/execute/commit phases as the interactive path.
pub fn batch_canonical(conn: &Connection, track_id: i64) -> Option<Canonical> {
    match canonical_from_metadata(conn, track_id) {
        Ok(Some(c)) => Some(c),
        Ok(None) => match reconcile_track(conn, track_id) {
            Ok(c) if c.confidence == naming::Confidence::Green => Some(c),
            _ => None,
        },
        Err(_) => None,
    }
}

/// Mark a track for re-sourcing (goes to Écartés, M4b): status `resourcing` + a `reject`
/// action. The file is not moved at this milestone.
pub fn reject_track(conn: &Connection, track_id: i64) -> Result<(), FilingError> {
    let source = track_path(conn, track_id)?;
    let batch_id = new_batch_id(track_id);
    // The journal row and the status flip are one fact, so they go in one transaction (same
    // `unchecked_transaction` reason as `commit_file`: we only hold `&Connection`). Dropping `tx`
    // on an early `?` rolls both back. No filesystem compensation to do — unlike filing and
    // trashing, rejecting moves nothing on disk.
    let tx = conn.unchecked_transaction()?;
    actions::record(
        conn,
        &batch_id,
        Some(track_id),
        "reject",
        Some(&source),
        None,
    )
    .map_err(|e| FilingError::Db(e.to_string()))?;
    conn.execute(
        "UPDATE tracks SET status='resourcing' WHERE id=?1",
        params![track_id],
    )?;
    tx.commit()?;
    Ok(())
}

/// Reject every track of `track_ids` for re-sourcing (each → Écartés, status-only like
/// `reject_track`). A track that errors is reported in `failed` rather than aborting the batch,
/// so one bad id never strands the rest — mirroring `file_batch`'s fail-soft, no-panic shape.
pub fn reject_batch(conn: &Connection, track_ids: &[i64]) -> RejectBatchResult {
    let mut rejected = 0usize;
    let mut failed = Vec::new();
    for &id in track_ids {
        match reject_track(conn, id) {
            Ok(()) => rejected += 1,
            Err(_) => failed.push(id),
        }
    }
    RejectBatchResult { rejected, failed }
}

// NOTE: the former one-shot `trash_track(conn, track_id)` is gone on purpose. It did the whole
// sequence — path lookup, byte-for-byte copy to the trash dir, journal, status flip — against a
// single `&Connection`, so `ipc_filing::trash_track` necessarily held the global connection mutex
// across the copy. It is now the explicit three phases `track_path` → `trash_file_fs` →
// `commit_trash`, which is the only way the copy can provably sit outside the lock.

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::run_migrations(&conn).unwrap();
        conn
    }

    fn fixture(name: &str) -> Option<String> {
        let p = format!("fixtures/{name}");
        if std::path::Path::new(&p).exists() {
            Some(p)
        } else {
            None
        }
    }

    /// Copy a fixture into `dir` and insert a pending track row pointing at the copy.
    fn seed_track(
        conn: &Connection,
        dir: &Path,
        fixture_name: &str,
        as_name: &str,
    ) -> Option<(i64, std::path::PathBuf)> {
        let src = fixture(fixture_name)?;
        let copy = dir.join(as_name);
        std::fs::copy(&src, &copy).unwrap();
        conn.execute(
            "INSERT INTO tracks(path, status) VALUES(?1, 'pending')",
            params![copy.to_str().unwrap()],
        )
        .unwrap();
        Some((conn.last_insert_rowid(), copy))
    }

    #[test]
    fn canonical_from_metadata_prefers_persisted_identity() {
        let conn = db();
        conn.execute(
            "INSERT INTO tracks(id, path, status) VALUES(1,'/x.flac','pending')",
            [],
        )
        .unwrap();
        // No metadata row → None (file_batch then falls back to the tag/filename reconcile).
        assert!(canonical_from_metadata(&conn, 1).unwrap().is_none());

        // A Discogs identity → a Green canonical on that name (what lets a per-track applied identity feed file_batch).
        conn.execute(
            "INSERT INTO metadata(track_id, artist, title, source) VALUES(1,'Larry Heard','Can You Feel It','discogs')",
            [],
        )
        .unwrap();
        let c = canonical_from_metadata(&conn, 1)
            .unwrap()
            .expect("metadata present");
        assert_eq!(c.artist, "Larry Heard");
        assert_eq!(c.title, "Can You Feel It");
        assert_eq!(c.confidence, crate::naming::Confidence::Green);

        // A blank-name row must be treated as absent (never file on an empty name).
        conn.execute(
            "UPDATE metadata SET artist='', title='' WHERE track_id=1",
            [],
        )
        .unwrap();
        assert!(canonical_from_metadata(&conn, 1).unwrap().is_none());
    }

    #[test]
    fn reconcile_track_reads_filename_when_tags_absent() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let Some((id, _)) = seed_track(
            &conn,
            dir.path(),
            "real_lossless.flac",
            "Robert Owens - Bring Down the Walls.flac",
        ) else {
            eprintln!("skip: no fixture");
            return;
        };
        let c = reconcile_track(&conn, id).unwrap();
        assert_eq!(c.artist, "Robert Owens");
        assert_eq!(c.title, "Bring Down the Walls");
    }

    fn seed_pioneer_dir_with_fixture(dir: &std::path::Path) -> std::path::PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::copy(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/rekordbox_master.db"
            ),
            dir.join("master.db"),
        )
        .unwrap();
        crate::actions::set_pioneer_dir_override_for_test(dir.to_path_buf());
        let xml_path = dir.join("masterPlaylists6.xml");
        std::fs::write(&xml_path, b"<DJ_PLAYLISTS/>").unwrap();
        xml_path
    }

    /// Patches the fixture's track_id "40000001" FolderPath to `path` — same technique as
    /// actions.rs's detect_masterdb_repair_ambiguous_on_two_matches test.
    fn patch_fixture_folder_path(pioneer_dir: &std::path::Path, path: &str) {
        let raw = std::fs::read(pioneer_dir.join("master.db")).unwrap();
        let plaintext = crate::rekordbox_masterdb::decrypt_masterdb_for_test(&raw);
        let len = plaintext.len();
        let mut conn2 = rusqlite::Connection::open_in_memory().unwrap();
        conn2
            .deserialize_read_exact(
                rusqlite::MAIN_DB,
                std::io::Cursor::new(plaintext),
                len,
                false,
            )
            .unwrap();
        conn2
            .execute(
                "UPDATE djmdContent SET FolderPath=?1 WHERE ID='40000001'",
                params![path],
            )
            .unwrap();
        let plaintext2 = conn2.serialize(rusqlite::MAIN_DB).unwrap().to_vec();
        let raw2 = crate::rekordbox_masterdb::encrypt_masterdb_for_test(&plaintext2);
        std::fs::write(pioneer_dir.join("master.db"), raw2).unwrap();
    }

    #[test]
    fn commit_file_conformant_detects_masterdb_metadata_sync() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("lib");
        std::fs::create_dir_all(root.join("House")).unwrap();
        let Some((id, src)) = seed_track(&conn, dir.path(), "real_320.mp3", "src.mp3") else {
            eprintln!("skip: no fixture");
            return;
        };

        let pioneer_dir = dir.path().join("pioneer");
        let xml_path = seed_pioneer_dir_with_fixture(&pioneer_dir);
        patch_fixture_folder_path(&pioneer_dir, src.to_str().unwrap());
        crate::settings::set(
            &conn,
            crate::settings::REKORDBOX_XML_PATH,
            xml_path.to_str().unwrap(),
        )
        .unwrap();

        let res = file_track(
            &conn,
            &root,
            "{artist} - {title}",
            id,
            "House",
            None,
            Some(Canonical {
                artist: "Larry Heard".into(),
                title: "Can You Feel It".into(),
                version: None,
                label: None,
                confidence: crate::naming::Confidence::Green,
            }),
            false,
        )
        .unwrap();
        let _ = res;

        let (new_artist, status): (Option<String>, String) = conn
            .query_row(
                "SELECT new_artist, status FROM rekordbox_masterdb_metadata_syncs WHERE track_id=?1",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("commit_file must have detected a metadata sync candidate");
        assert_eq!(status, "pending");
        assert_eq!(new_artist.as_deref(), Some("Larry Heard"));
    }

    #[test]
    fn commit_file_non_conformant_detects_masterdb_metadata_sync() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("lib");
        std::fs::create_dir_all(root.join("House")).unwrap();
        let Some((id, src)) = seed_track(&conn, dir.path(), "real_lossless.flac", "src.flac")
        else {
            eprintln!("skip: no fixture");
            return;
        };
        crate::ffmpeg::init_ffmpeg_path();

        let pioneer_dir = dir.path().join("pioneer");
        let xml_path = seed_pioneer_dir_with_fixture(&pioneer_dir);
        patch_fixture_folder_path(&pioneer_dir, src.to_str().unwrap());
        crate::settings::set(
            &conn,
            crate::settings::REKORDBOX_XML_PATH,
            xml_path.to_str().unwrap(),
        )
        .unwrap();

        let res = file_track(
            &conn,
            &root,
            "{artist} - {title}",
            id,
            "House",
            None,
            Some(Canonical {
                artist: "Theo Parrish".into(),
                title: "Falling Up".into(),
                version: None,
                label: None,
                confidence: crate::naming::Confidence::Green,
            }),
            false,
        )
        .unwrap();
        let _ = res;

        let (new_artist, status): (Option<String>, String) = conn
            .query_row(
                "SELECT new_artist, status FROM rekordbox_masterdb_metadata_syncs WHERE track_id=?1",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("commit_file must have detected a metadata sync candidate on the non-conformant (convert) path");
        assert_eq!(status, "pending");
        assert_eq!(new_artist.as_deref(), Some("Theo Parrish"));
    }

    #[test]
    fn files_conformant_mp3_by_moving() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("lib");
        std::fs::create_dir_all(root.join("House")).unwrap();
        let Some((id, src)) = seed_track(&conn, dir.path(), "real_320.mp3", "src.mp3") else {
            eprintln!("skip: no fixture");
            return;
        };

        let res = file_track(
            &conn,
            &root,
            "{artist} - {title}",
            id,
            "House",
            None,
            Some(Canonical {
                artist: "Larry Heard".into(),
                title: "Can You Feel It".into(),
                version: None,
                label: None,
                confidence: crate::naming::Confidence::Green,
            }),
            false,
        )
        .unwrap();

        // moved into bin, original gone (mono-location), one move action
        assert!(std::path::Path::new(&res.path).exists());
        assert!(!src.exists());
        assert!(res.path.ends_with("Larry Heard - Can You Feel It.mp3"));
        let (status, folder): (String, Option<String>) = conn
            .query_row(
                "SELECT status, folder FROM tracks WHERE id=?1",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(status, "filed");
        assert_eq!(folder.as_deref(), Some("House"));
        let moves: i64 = conn
            .query_row(
                "SELECT count(*) FROM actions WHERE type='move' AND undone=0",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(moves, 1);
    }

    /// Reverting a CONFORMANT filing must remove the Discogs tags FROM THE FILE (restore the old
    /// ones), not just move the file back — else the file still carries the applied tags and the B9
    /// "not written" marker would wrongly stay hidden. The conformant filing journals tag_edit+move;
    /// revert undoes the move (file → source) THEN restores the captured old tags at source.
    #[test]
    fn revert_of_conformant_filing_restores_old_file_tags() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("lib");
        std::fs::create_dir_all(root.join("House")).unwrap();
        let Some((id, src)) = seed_track(&conn, dir.path(), "real_320.mp3", "src.mp3") else {
            eprintln!("skip: no fixture");
            return;
        };
        // Give the source file KNOWN old tags before filing.
        crate::tagging::write_tags_full(
            src.to_str().unwrap(),
            "OLD Artist",
            "OLD Title",
            None,
            None,
            &[],
            None,
        )
        .unwrap();

        let res = file_track(
            &conn,
            &root,
            "{artist} - {title}",
            id,
            "House",
            None,
            Some(Canonical {
                artist: "NEW Artist".into(),
                title: "NEW Title".into(),
                version: None,
                label: None,
                confidence: crate::naming::Confidence::Green,
            }),
            false,
        )
        .unwrap();
        // Filed: file moved into the bin, carrying the NEW tags.
        let after = crate::tagging::read_tags_full(&res.path).unwrap();
        assert_eq!(
            after.artist.as_deref(),
            Some("NEW Artist"),
            "filing wrote the new tags"
        );

        // Revert the whole filing batch (move undone, then old tags restored).
        crate::actions::revert_batch(&conn, &res.batch_id).unwrap();

        assert!(src.exists(), "the file is moved back to its source");
        assert!(
            !std::path::Path::new(&res.path).exists(),
            "nothing left at the bin destination"
        );
        let restored = crate::tagging::read_tags_full(src.to_str().unwrap()).unwrap();
        assert_eq!(
            restored.artist.as_deref(),
            Some("OLD Artist"),
            "old file tags restored on revert"
        );
        assert_eq!(restored.title.as_deref(), Some("OLD Title"));
    }

    /// CR-3 (audit multi-passes du 2026-07-28) — un échec du `move` sur le chemin CONFORMANT
    /// laissait les nouveaux tags écrits en place sur le fichier source, sans rien pour les
    /// défaire.
    ///
    /// Le chemin conformant tague le fichier À SA PLACE puis le déplace. Si le déplacement échoue
    /// (disque plein, destination verrouillée, permission, dossier disparu), la fonction sortait
    /// par un `?` nu : le fichier restait à sa source, porteur de tags que l'utilisateur n'avait
    /// pas demandés, et SANS ligne de journal — le journal n'est écrit qu'en phase 3, depuis le
    /// `log` retourné. Donc aucun revert possible depuis l'app, et aucune trace. La ligne
    /// `tag_edit` avec l'instantané des anciens tags existait pourtant déjà dans `log`, poussée
    /// avant l'écriture précisément pour ce cas ; personne ne la rejouait.
    ///
    /// L'échec est provoqué en supprimant le dossier de destination APRÈS le plan : `std::fs::rename`
    /// échoue alors sur un chemin introuvable, ce qui n'est ni 17 ni 18 et ne déclenche donc pas le
    /// repli copy_verify_delete — l'erreur remonte, comme un vrai échec disque.
    #[test]
    fn move_failure_restores_the_tags_it_had_already_overwritten() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("lib");
        std::fs::create_dir_all(root.join("House")).unwrap();
        let Some((id, src)) = seed_track(&conn, dir.path(), "real_320.mp3", "src.mp3") else {
            eprintln!("skip: no fixture");
            return;
        };
        crate::tagging::write_tags_full(
            src.to_str().unwrap(),
            "OLD Artist",
            "OLD Title",
            None,
            None,
            &[],
            None,
        )
        .unwrap();

        let plan = plan_file(
            &conn,
            Some(&root),
            "{artist} - {title}",
            id,
            "House",
            None,
            Some(Canonical {
                artist: "NEW Artist".into(),
                title: "NEW Title".into(),
                version: None,
                label: None,
                confidence: crate::naming::Confidence::Green,
            }),
            false,
            &HashSet::new(),
        )
        .unwrap();
        assert!(
            plan.conformant,
            "ce test ne vaut que pour le chemin conformant (tag en place puis move)"
        );

        // Fait échouer le déplacement, après que le plan a figé la destination.
        std::fs::remove_dir_all(root.join("House")).unwrap();

        // `FsLog` ne derive pas Debug (et ce n'est pas a ce test de le lui ajouter): on teste donc
        // la variante d'erreur, pas la valeur complète.
        let err = execute_file(&plan).err();
        assert!(
            err.is_some(),
            "le déplacement doit échouer une fois le dossier de destination supprimé"
        );

        assert!(
            src.exists(),
            "le fichier source doit être encore là: rien ne l'a déplacé"
        );
        assert!(
            !std::path::Path::new(&plan.dest).exists(),
            "rien ne doit avoir été écrit à la destination"
        );
        let after = crate::tagging::read_tags_full(src.to_str().unwrap()).unwrap();
        assert_eq!(
            after.artist.as_deref(),
            Some("OLD Artist"),
            "les tags écrits avant le move raté doivent avoir été défaits: le fichier de \
             l'utilisateur ne doit pas garder des tags issus d'un rangement qui n'a pas eu lieu"
        );
        assert_eq!(after.title.as_deref(), Some("OLD Title"));
    }

    /// FIX-15: `rollback_fs` (the "nothing is left half-filed" guarantee) had no test forcing
    /// `commit_file` to fail AFTER `execute_file` already moved the file. Deleting the track row
    /// between the two phases makes the actions insert's FK (track_id REFERENCES tracks(id))
    /// fail — the same shape of failure `commit_file` guards against (a real DB error mid-commit).
    #[test]
    fn commit_failure_rolls_back_the_conformant_move() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("lib");
        std::fs::create_dir_all(root.join("House")).unwrap();
        let Some((id, src)) = seed_track(&conn, dir.path(), "real_320.mp3", "src.mp3") else {
            eprintln!("skip: no fixture");
            return;
        };
        let plan = plan_file(
            &conn,
            Some(&root),
            "{artist} - {title}",
            id,
            "House",
            None,
            Some(Canonical {
                artist: "Larry Heard".into(),
                title: "Can You Feel It".into(),
                version: None,
                label: None,
                confidence: crate::naming::Confidence::Green,
            }),
            false,
            &HashSet::new(),
        )
        .unwrap();
        let log = execute_file(&plan).unwrap();
        // Phase 2 already ran: the file really moved.
        assert!(!src.exists());
        assert!(std::path::Path::new(&plan.dest).exists());

        conn.execute("DELETE FROM tracks WHERE id=?1", params![id])
            .unwrap();
        assert!(
            commit_file(&conn, &plan, log, None, None).is_err(),
            "commit must fail once its track row is gone"
        );

        // Nothing left half-filed: the file is back at its original path, gone from the bin.
        assert!(
            src.exists(),
            "rollback must restore the file at its original path"
        );
        assert!(
            !std::path::Path::new(&plan.dest).exists(),
            "rollback must remove it from the bin"
        );
    }

    #[test]
    fn files_flac_by_converting_to_aiff_and_trashing_original() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("lib");
        std::fs::create_dir_all(root.join("House")).unwrap();
        let Some((id, src)) = seed_track(&conn, dir.path(), "real_lossless.flac", "src.flac")
        else {
            eprintln!("skip: no fixture");
            return;
        };
        crate::ffmpeg::init_ffmpeg_path();

        let res = file_track(
            &conn,
            &root,
            "{artist} - {title}",
            id,
            "House",
            None,
            Some(Canonical {
                artist: "Theo Parrish".into(),
                title: "Falling Up".into(),
                version: None,
                label: None,
                confidence: crate::naming::Confidence::Green,
            }),
            false,
        )
        .unwrap();

        // converted AIFF lands in the bin; conformant to target
        assert!(res.path.ends_with("Theo Parrish - Falling Up.aiff"));
        assert!(crate::encode::is_conformant(
            &res.path,
            crate::encode::Target::Aiff1644
        ));
        // original is in .sift-trash, not at its source location (mono-location)
        assert!(!src.exists());
        let convert_rows: i64 = conn
            .query_row(
                "SELECT count(*) FROM actions WHERE type='convert' AND undone=0",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let trash_rows: i64 = conn
            .query_row(
                "SELECT count(*) FROM actions WHERE type='trash' AND undone=0",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(convert_rows, 1);
        assert_eq!(trash_rows, 1);
    }

    /// Root fix for the `.aif`/`.aiff` revert-duplicate: a CONFORMANT AIFF is moved (no transcode),
    /// so it must keep its own extension instead of being forced to the canonical `.aiff`. We build a
    /// conformant 3-letter `.aif` by encoding the lossless fixture to AIFF 16/44.1, then file it and
    /// assert the destination stays `.aif` and the action was a `move` (not `convert`). With a single
    /// possible output name, a later blocked revert can no longer leave a `.aif` next to a `.aiff`.
    #[test]
    fn files_conformant_aif_preserving_its_extension() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("lib");
        std::fs::create_dir_all(root.join("House")).unwrap();
        let Some(flac) = fixture("real_lossless.flac") else {
            eprintln!("skip: no fixture");
            return;
        };
        crate::ffmpeg::init_ffmpeg_path();

        // A conformant source whose extension is the 3-letter `.aif` (the case formerly forced to `.aiff`).
        let aif_src = dir.path().join("src.aif");
        crate::encode::encode(
            &flac,
            aif_src.to_str().unwrap(),
            crate::encode::Target::Aiff1644,
        )
        .unwrap();
        assert!(
            crate::encode::is_conformant(
                aif_src.to_str().unwrap(),
                crate::encode::Target::Aiff1644
            ),
            "the built .aif is conformant"
        );
        conn.execute(
            "INSERT INTO tracks(path, status) VALUES(?1, 'pending')",
            params![aif_src.to_str().unwrap()],
        )
        .unwrap();
        let id = conn.last_insert_rowid();

        let res = file_track(
            &conn,
            &root,
            "{artist} - {title}",
            id,
            "House",
            None,
            Some(Canonical {
                artist: "Larry Heard".into(),
                title: "Can You Feel It".into(),
                version: None,
                label: None,
                confidence: crate::naming::Confidence::Green,
            }),
            false,
        )
        .unwrap();

        // Moved keeping `.aif` — NOT forced to the 4-letter `.aiff`.
        assert!(
            res.path.ends_with("Larry Heard - Can You Feel It.aif"),
            "dest keeps .aif: {}",
            res.path
        );
        assert!(
            !res.path.ends_with(".aiff"),
            "must not force .aiff on a moved conformant file"
        );
        assert!(std::path::Path::new(&res.path).exists());
        assert!(!aif_src.exists(), "moved out of source (mono-location)");
        // It was a pure MOVE: no conversion, no trash.
        let moves: i64 = conn
            .query_row(
                "SELECT count(*) FROM actions WHERE type='move' AND undone=0",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let converts: i64 = conn
            .query_row(
                "SELECT count(*) FROM actions WHERE type='convert' AND undone=0",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(moves, 1, "conformant .aif is moved");
        assert_eq!(converts, 0, "no conversion for an already-conformant file");
    }

    #[test]
    fn file_track_refuses_lossy_to_aiff() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("lib");
        std::fs::create_dir_all(&root).unwrap();
        let Some((id, _)) = seed_track(&conn, dir.path(), "real_320.mp3", "src.mp3") else {
            eprintln!("skip: no fixture");
            return;
        };
        let err = file_track(
            &conn,
            &root,
            "{artist} - {title}",
            id,
            "",
            Some(Target::Aiff1644),
            Some(Canonical {
                artist: "X".into(),
                title: "Y".into(),
                version: None,
                label: None,
                confidence: crate::naming::Confidence::Green,
            }),
            false,
        );
        assert_eq!(err, Err(FilingError::Upscale));
    }

    #[test]
    fn plan_file_external_dest_resolves_outside_root_when_directory_exists() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("lib");
        std::fs::create_dir_all(&root).unwrap();
        let external = dir.path().join("elsewhere");
        std::fs::create_dir_all(&external).unwrap();
        let Some((id, _)) = seed_track(&conn, dir.path(), "real_320.mp3", "src.mp3") else {
            eprintln!("skip: no fixture");
            return;
        };
        let bin_rel = format!("{EXTERNAL_DEST_PREFIX}{}", external.to_str().unwrap());
        let plan = plan_file(
            &conn,
            Some(&root),
            "{artist} - {title}",
            id,
            &bin_rel,
            None,
            Some(Canonical {
                artist: "X".into(),
                title: "Y".into(),
                version: None,
                label: None,
                confidence: crate::naming::Confidence::Green,
            }),
            false,
            &HashSet::new(),
        )
        .unwrap();
        let dest = Path::new(&plan.dest);
        assert!(
            dest.starts_with(&external),
            "dest {dest:?} should land under the external dir, not the library root"
        );
        assert!(
            !dest.starts_with(&root),
            "dest {dest:?} must NOT be under the library root"
        );
    }

    #[test]
    fn plan_file_external_dest_fails_loudly_when_directory_is_gone() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("lib");
        std::fs::create_dir_all(&root).unwrap();
        let Some((id, _)) = seed_track(&conn, dir.path(), "real_320.mp3", "src.mp3") else {
            eprintln!("skip: no fixture");
            return;
        };
        let missing = dir.path().join("never-created");
        let bin_rel = format!("{EXTERNAL_DEST_PREFIX}{}", missing.to_str().unwrap());
        let err = plan_file(
            &conn,
            Some(&root),
            "{artist} - {title}",
            id,
            &bin_rel,
            None,
            Some(Canonical {
                artist: "X".into(),
                title: "Y".into(),
                version: None,
                label: None,
                confidence: crate::naming::Confidence::Green,
            }),
            false,
            &HashSet::new(),
        );
        assert_eq!(
            err.err(),
            Some(FilingError::Io(format!(
                "external destination no longer exists: {}",
                missing.to_str().unwrap()
            )))
        );
    }

    /// #54 — la racine est une exigence de l'ARBRE. Table de vérité de la fonction qui porte la
    /// distinction : elle décide, à elle seule, quelles destinations réclament encore une racine.
    #[test]
    fn needs_library_root_only_for_tree_destinations() {
        assert!(needs_library_root(""), "la racine elle-même est un bac");
        assert!(needs_library_root("House/Deep"));
        assert!(!needs_library_root(FILE_IN_PLACE));
        assert!(!needs_library_root(&format!(
            "{EXTERNAL_DEST_PREFIX}/tmp/x"
        )));
        // Un bac qui NOMME la sentinelle sans l'être reste un bac de l'arbre.
        assert!(needs_library_root("__SOURCE__ bis"));
        // TRAVERSÉES — le point qui compte pour la sécurité : une tentative de sortie de la racine
        // doit rester un chemin d'ARBRE, donc passer par `safe_join`, qui les rejette (ses propres
        // tests l'épinglent). Si `needs_library_root` les classait « sans racine », elles
        // contourneraient `safe_join` en entier et le trou anti-traversal serait total.
        assert!(needs_library_root("../evil"));
        assert!(needs_library_root("House/../../x"));
        assert!(needs_library_root("..\\evil"));
        assert!(needs_library_root("/etc"));
        assert!(needs_library_root("C:\\Windows"));
    }

    /// La branche externe est LE chemin de rangement sans racine depuis #54 : son contrat
    /// « chemin absolu venu du dialog natif » devient exécutable. Un relatif résoudrait contre le
    /// répertoire courant du process — ni la bibliothèque, ni un dossier désigné par l'utilisateur.
    #[test]
    fn plan_file_external_dest_refuses_a_relative_path() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let Some((id, _)) = seed_track(&conn, dir.path(), "real_320.mp3", "src.mp3") else {
            eprintln!("skip: no fixture");
            return;
        };
        // Un relatif qui EXISTE réellement sous le cwd (`src/`, depuis src-tauri) : le refus doit
        // venir du caractère relatif, pas d'une absence de dossier — sinon le garde ne prouve rien.
        assert!(
            std::path::Path::new("src").is_dir(),
            "témoin: le test suppose un cwd = src-tauri, où `src/` existe"
        );
        let err = plan_file(
            &conn,
            None,
            "{artist} - {title}",
            id,
            &format!("{EXTERNAL_DEST_PREFIX}src"),
            None,
            Some(Canonical {
                artist: "X".into(),
                title: "Y".into(),
                version: None,
                label: None,
                confidence: crate::naming::Confidence::Green,
            }),
            false,
            &HashSet::new(),
        );
        assert_eq!(
            err.err(),
            Some(FilingError::Io(
                "external destination must be an absolute path: src".to_string()
            ))
        );
    }

    /// La chaîne de la sentinelle est le contrat lu par le front (`filing-actions.ts`,
    /// `batch-panel.ts`) : elle est épinglée ici, pas seulement produite.
    #[test]
    fn no_library_root_displays_the_contract_sentinel() {
        assert_eq!(FilingError::NoLibraryRoot.to_string(), "NoLibraryRoot");
    }

    /// #54, cœur du volet backend : SANS racine, un rangement EN PLACE planifie normalement.
    /// Avant, la racine était réclamée à l'entrée de la commande et ce plan n'existait pas.
    #[test]
    fn plan_file_in_place_needs_no_library_root() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let Some((id, src)) = seed_track(&conn, dir.path(), "real_320.mp3", "src.mp3") else {
            eprintln!("skip: no fixture");
            return;
        };
        let plan = plan_file(
            &conn,
            None,
            "{artist} - {title}",
            id,
            FILE_IN_PLACE,
            None,
            Some(Canonical {
                artist: "X".into(),
                title: "Y".into(),
                version: None,
                label: None,
                confidence: crate::naming::Confidence::Green,
            }),
            false,
            &HashSet::new(),
        )
        .expect("un rangement en place ne dépend pas de la racine");
        assert_eq!(
            Path::new(&plan.dest).parent(),
            Path::new(&src).parent(),
            "en place = le dossier de la source"
        );
    }

    /// Même absence de racine, destination externe : planifie aussi.
    #[test]
    fn plan_file_external_dest_needs_no_library_root() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let external = dir.path().join("elsewhere");
        std::fs::create_dir_all(&external).unwrap();
        let Some((id, _)) = seed_track(&conn, dir.path(), "real_320.mp3", "src.mp3") else {
            eprintln!("skip: no fixture");
            return;
        };
        let bin_rel = format!("{EXTERNAL_DEST_PREFIX}{}", external.to_str().unwrap());
        let plan = plan_file(
            &conn,
            None,
            "{artist} - {title}",
            id,
            &bin_rel,
            None,
            Some(Canonical {
                artist: "X".into(),
                title: "Y".into(),
                version: None,
                label: None,
                confidence: crate::naming::Confidence::Green,
            }),
            false,
            &HashSet::new(),
        )
        .expect("un dossier externe ne dépend pas de la racine");
        assert!(Path::new(&plan.dest).starts_with(&external));
    }

    /// L'autre moitié du contrat : viser l'ARBRE sans racine reste refusé, et par la sentinelle.
    #[test]
    fn plan_file_into_tree_without_root_is_refused() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let Some((id, _)) = seed_track(&conn, dir.path(), "real_320.mp3", "src.mp3") else {
            eprintln!("skip: no fixture");
            return;
        };
        for bin_rel in ["", "House/Deep"] {
            let err = plan_file(
                &conn,
                None,
                "{artist} - {title}",
                id,
                bin_rel,
                None,
                Some(Canonical {
                    artist: "X".into(),
                    title: "Y".into(),
                    version: None,
                    label: None,
                    confidence: crate::naming::Confidence::Green,
                }),
                false,
                &HashSet::new(),
            );
            assert_eq!(
                err.err(),
                Some(FilingError::NoLibraryRoot),
                "bac {bin_rel:?}"
            );
        }
    }

    #[test]
    fn reject_track_sets_resourcing_and_records() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let Some((id, _)) = seed_track(&conn, dir.path(), "real_320.mp3", "src.mp3") else {
            eprintln!("skip: no fixture");
            return;
        };
        reject_track(&conn, id).unwrap();
        let status: String = conn
            .query_row("SELECT status FROM tracks WHERE id=?1", params![id], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(status, "resourcing");
        let rejects: i64 = conn
            .query_row(
                "SELECT count(*) FROM actions WHERE type='reject'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(rejects, 1);
    }

    /// `reject_track` écrit DEUX lignes : le journal, puis le statut. Sans transaction, un statut
    /// refusé laissait une action `reject` orpheline — le journal annonçait un écart qui n'a
    /// jamais eu lieu, et « Annuler » proposait de défaire une action fantôme. Pas de fixture ici :
    /// `track_path` ne lit que la colonne, le fichier n'a pas besoin d'exister.
    #[test]
    fn reject_track_ne_laisse_pas_d_action_orpheline_si_le_statut_est_refuse() {
        let conn = db();
        conn.execute(
            "INSERT INTO tracks(path, status) VALUES('orphelin.mp3','pending')",
            [],
        )
        .unwrap();
        let id = conn.last_insert_rowid();
        // Ce trigger refuse la SECONDE écriture seulement, jamais la première.
        conn.execute_batch(
            "CREATE TRIGGER refuse_resourcing BEFORE UPDATE OF status ON tracks
             WHEN NEW.status='resourcing'
             BEGIN SELECT RAISE(ABORT, 'refus de test'); END;",
        )
        .unwrap();

        assert!(reject_track(&conn, id).is_err());
        let rejects: i64 = conn
            .query_row(
                "SELECT count(*) FROM actions WHERE type='reject'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            rejects, 0,
            "action `reject` journalisée alors que la piste n'a jamais changé de statut"
        );
    }

    #[test]
    fn reject_batch_marks_all_and_collects_bad_ids() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let (Some((a, _)), Some((b, _))) = (
            seed_track(&conn, dir.path(), "real_320.mp3", "a.mp3"),
            seed_track(&conn, dir.path(), "real_320.mp3", "b.mp3"),
        ) else {
            eprintln!("skip: no fixture");
            return;
        };
        // 999 is not a real track id → reject_track errors → reported in `failed`, batch not aborted.
        let res = reject_batch(&conn, &[a, b, 999]);
        assert_eq!(
            res,
            RejectBatchResult {
                rejected: 2,
                failed: vec![999]
            }
        );
        let resourced: i64 = conn
            .query_row(
                "SELECT count(*) FROM tracks WHERE status='resourcing'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(resourced, 2);
    }

    #[test]
    fn trash_track_moves_to_sift_trash() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let Some((id, src)) = seed_track(&conn, dir.path(), "real_320.mp3", "src.mp3") else {
            eprintln!("skip: no fixture");
            return;
        };
        // The three phases in the order `ipc_filing::trash_track` runs them (path under the lock,
        // copy off it, journal + status back under it).
        let source = track_path(&conn, id).unwrap();
        let dest = trash_file_fs(id, &source).unwrap();
        commit_trash(&conn, id, &source, &dest).unwrap();
        assert!(!src.exists());
        let status: String = conn
            .query_row("SELECT status FROM tracks WHERE id=?1", params![id], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(status, "trash");
        // Trash is centralized to <Documents>/Sift/Trash/ (cross-disk safe), not under `root`.
        // The moved file is named `<track_id>__<original_name>` there (ensure_unique may suffix it
        // if a prior run left one behind, so match on the `<id>__` prefix, not an exact name).
        let trash_dir = sift_trash_dir().unwrap();
        let prefix = format!("{id}__");
        let entries: Vec<std::path::PathBuf> = trash_dir
            .read_dir()
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with(&prefix))
            })
            .collect();
        assert!(
            !entries.is_empty(),
            "trashed file should land in the central Sift trash dir"
        );
        for p in entries {
            std::fs::remove_file(&p).ok();
        }
    }

    #[test]
    fn filing_writes_applied_genres_to_the_file() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("lib");
        std::fs::create_dir_all(root.join("House")).unwrap();
        let Some((id, _src)) = seed_track(&conn, dir.path(), "real_320.mp3", "src.mp3") else {
            eprintln!("skip: no fixture");
            return;
        };

        // seed a Discogs identity for the track BEFORE filing
        let cand = crate::metadata::Candidate {
            artist: "Larry Heard".into(),
            title: "Mystery of Love".into(),
            label: Some("Alleviated".into()),
            year: Some(1986),
            styles: vec!["Deep House".into()],
            country: None,
            format: None,
            cover_url: None,
            release_id: "12345".into(),
            source: "discogs".into(),
        };
        crate::metadata::apply_identity(&conn, id, &cand, None).unwrap();

        let res = file_track(
            &conn,
            &root,
            "{artist} - {title}",
            id,
            "House",
            None,
            Some(Canonical {
                artist: "Larry Heard".into(),
                title: "Mystery of Love".into(),
                version: None,
                label: None,
                confidence: crate::naming::Confidence::Green,
            }),
            false,
        )
        .unwrap();

        use lofty::file::TaggedFileExt;
        use lofty::probe::Probe;
        use lofty::tag::ItemKey;
        let tagged = Probe::open(&res.path).unwrap().read().unwrap();
        let tag = tagged.primary_tag().unwrap();
        let genre = tag.get_string(ItemKey::Genre).unwrap_or("");
        assert!(
            genre.contains("Deep House"),
            "filed file has applied genre; got {genre:?}"
        );
    }

    /// FIX-1 (BUG-1): an MP3 disguised with a `.flac` extension must be REFUSED by default
    /// (`RailMismatch`, not silently converted into a fabricated lossless AIFF), and must succeed
    /// once the caller explicitly passes `allow_rail_mismatch=true` (the confirmed-by-the-user
    /// path). A genuine FLAC must file normally either way — no false positive.
    #[test]
    fn plan_file_blocks_a_disguised_lossy_source_unless_allowed() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("lib");
        std::fs::create_dir_all(&root).unwrap();
        let Some(mp3) = fixture("real_320.mp3") else {
            eprintln!("skip: no fixture");
            return;
        };
        let disguised = dir.path().join("disguised.flac");
        std::fs::copy(&mp3, &disguised).unwrap();
        conn.execute(
            "INSERT INTO tracks(path, status) VALUES(?1, 'pending')",
            params![disguised.to_str().unwrap()],
        )
        .unwrap();
        let id = conn.last_insert_rowid();
        let canonical = Some(Canonical {
            artist: "X".into(),
            title: "Y".into(),
            version: None,
            label: None,
            confidence: crate::naming::Confidence::Green,
        });

        // Default (allow_rail_mismatch=false): refused, nothing touched.
        let blocked = plan_file(
            &conn,
            Some(&root),
            "{artist} - {title}",
            id,
            "House",
            None,
            canonical.clone(),
            false,
            &HashSet::new(),
        );
        assert_eq!(blocked.err(), Some(FilingError::RailMismatch));
        assert!(
            disguised.exists(),
            "refused plan must not touch the source file"
        );

        // Explicit confirmation (allow_rail_mismatch=true): proceeds normally.
        crate::ffmpeg::init_ffmpeg_path();
        let allowed = plan_file(
            &conn,
            Some(&root),
            "{artist} - {title}",
            id,
            "House",
            None,
            canonical,
            true,
            &HashSet::new(),
        );
        assert!(
            allowed.is_ok(),
            "an explicitly confirmed mismatch must proceed: {:?}",
            allowed.err()
        );
    }

    /// No false positive: a genuine FLAC must never trip the mismatch guard.
    #[test]
    fn plan_file_does_not_flag_a_genuine_lossless_source() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("lib");
        std::fs::create_dir_all(&root).unwrap();
        let Some((id, _src)) = seed_track(&conn, dir.path(), "real_lossless.flac", "src.flac")
        else {
            eprintln!("skip: no fixture");
            return;
        };
        crate::ffmpeg::init_ffmpeg_path();
        let res = plan_file(
            &conn,
            Some(&root),
            "{artist} - {title}",
            id,
            "House",
            None,
            Some(Canonical {
                artist: "X".into(),
                title: "Y".into(),
                version: None,
                label: None,
                confidence: crate::naming::Confidence::Green,
            }),
            false,
            &HashSet::new(),
        );
        assert!(
            res.is_ok(),
            "a genuine FLAC must not be blocked: {:?}",
            res.err()
        );
    }

    /// Régression vécue le 2026-07-30 : une piste rangée depuis un dossier de téléchargement vers
    /// la racine de bibliothèque a bien été déplacée ET renommée sur le disque, mais
    /// `tracks.path` a gardé le chemin SOURCE — devenu inexistant. La Bibliothèque lit ce champ
    /// (`library::list_filed` sélectionne `t.path`) : elle pointait donc un fichier absent, et la
    /// piste ne se lisait pas. Le journal, lui, avait correctement les deux chemins.
    ///
    /// Cause : `commit_file` écrivait `status`/`folder`/`target_format`/`confidence` et JAMAIS
    /// `path`. Rien ne rattrapait la ligne — `scanner::forget_path` ne supprime que les `pending`.
    ///
    /// CORRIGÉ le 2026-07-31 — ce test a été `#[ignore]` un temps, et l'historique de ce refus vaut
    /// d'être gardé : le correctif évident (ajouter `path=?` à l'`UPDATE` de `commit_file`) avait
    /// été écrit puis RETIRÉ, le crosscheck de la gate ayant montré qu'il casse deux choses pires
    /// que le bug. La leçon tient toujours : les trois conditions ci-dessous sont INDISSOCIABLES —
    /// remplir la première seule fait dépasser le remède sur le mal. Elles sont aujourd'hui
    /// SATISFAITES, chacune par son propre test ; toucher l'une sans les autres rouvre son défaut.
    ///
    /// 1. **Le revert restaure `path`** (+ `filename`/`size_bytes`/`mtime`).
    ///    `actions::revert_batch` remettait `status='pending', folder=NULL, target_format=NULL,
    ///    confidence=NULL` sans toucher `path` — cohérent tant que `path` restait sur la source,
    ///    mais dès qu'il pointe la destination, une piste annulée garde le chemin d'un fichier que
    ///    `revert_one_fs` vient de rendre à sa source : le bug, en miroir. Le chemin d'origine se
    ///    relit sur le journal (`from_path` de la ligne `move`/`convert` la plus ancienne du lot),
    ///    seule copie qui subsiste une fois `path` repointé.
    ///    → `revert_batch_ramene_le_chemin_et_les_metadonnees_sur_la_source`.
    /// 2. **`size_bytes`, `mtime` et `filename` suivent le déplacement.** Un rangement EN PLACE
    ///    (`FILE_IN_PLACE`) résout `dest_dir` vers `source.parent()` — donc un dossier SURVEILLÉ.
    ///    Au passage suivant du watcher, `scanner::upsert_file` trouvait une ligne à ce chemin dont
    ///    la taille et la date étaient celles d'AVANT l'écriture des tags, prenait la branche
    ///    `Some(_)` (`scanner.rs`) et repassait la piste en `status='pending'` en effaçant
    ///    `analyzed_at`, `fingerprint` et `report_json`. La piste rangée se dérangeait toute seule.
    ///    → `un_rescan_du_fichier_range_en_place_ne_le_derange_pas`.
    /// 3. **La collision `UNIQUE(path)` est traitée explicitement.** Entre le déplacement et la
    ///    transaction, le watcher peut avoir INSÉRÉ une ligne `pending` pour le fichier à sa
    ///    destination ; l'`UPDATE` violait alors la contrainte, `rollback_fs` défaisait le
    ///    déplacement et le rangement échouait pour l'utilisateur. Cette ligne parasite est
    ///    évincée dans la transaction (DELETE étroit : `pending` ET `id<>` la nôtre) ; ce qui y
    ///    survit est une piste DÉJÀ rangée à ce chemin, donc un vrai conflit métier → `DestOccupied`.
    ///    → `commit_file_evince_la_ligne_pending_concurrente_a_la_destination` et
    ///    `commit_file_refuse_une_destination_occupee_par_une_piste_rangee`.
    #[test]
    fn commit_file_repointe_le_chemin_sur_la_destination() {
        let conn = db();
        conn.execute(
            "INSERT INTO tracks(path, status) VALUES('D:/DL/12 - vieux nom.aif', 'pending')",
            [],
        )
        .unwrap();
        let track_id = conn.last_insert_rowid();

        let plan = FilePlan {
            track_id,
            batch_id: "b-path".to_string(),
            source: "D:/DL/12 - vieux nom.aif".to_string(),
            dest: "D:/KEPT/Artiste - Titre (Club Mix).aif".to_string(),
            conformant: true,
            target: Target::Aiff1644,
            canonical: Canonical {
                artist: "Artiste".to_string(),
                title: "Titre".to_string(),
                version: Some("Club Mix".to_string()),
                label: None,
                confidence: naming::Confidence::Green,
            },
            bin_rel: String::new(),
            extras: TagExtras {
                label: None,
                year: None,
                genres: vec![],
                cover_path: None,
            },
        };
        let log = vec![FsLog {
            kind: "move",
            from: plan.source.clone(),
            to: plan.dest.clone(),
            meta: None,
        }];

        commit_file(&conn, &plan, log, None, None).expect("commit_file");

        let (path, status): (String, String) = conn
            .query_row(
                "SELECT path, status FROM tracks WHERE id=?1",
                params![track_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(status, "filed");
        assert_eq!(
            path, "D:/KEPT/Artiste - Titre (Club Mix).aif",
            "tracks.path doit suivre le fichier, sinon la Bibliothèque pointe un fichier absent"
        );
    }

    /// Plan minimal pour les tests de `commit_file` : seuls `track_id`, `batch_id`, `source` et
    /// `dest` portent du sens ici (le reste alimente `save_metadata` et les colonnes de filing).
    fn commit_plan_stub(track_id: i64, batch_id: &str, source: &str, dest: &str) -> FilePlan {
        FilePlan {
            track_id,
            batch_id: batch_id.to_string(),
            source: source.to_string(),
            dest: dest.to_string(),
            conformant: true,
            target: Target::Aiff1644,
            canonical: Canonical {
                artist: "Artiste".to_string(),
                title: "Titre".to_string(),
                version: Some("Club Mix".to_string()),
                label: None,
                confidence: naming::Confidence::Green,
            },
            bin_rel: String::new(),
            extras: TagExtras {
                label: None,
                year: None,
                genres: vec![],
                cover_path: None,
            },
        }
    }

    /// Condition (a) du correctif de `commit_file_repointe_le_chemin_sur_la_destination` : dès que
    /// `path` suit le fichier, l'annulation doit le ramener sur la SOURCE — sinon c'est le même bug
    /// en miroir, `revert_one_fs` ayant remis le fichier à sa place d'origine. `size_bytes`, `mtime`
    /// et `filename` suivent le même aller-retour (un converti n'a ni la taille ni le nom de son
    /// original).
    #[test]
    fn revert_batch_ramene_le_chemin_et_les_metadonnees_sur_la_source() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let dl = dir.path().join("DL");
        let kept = dir.path().join("KEPT");
        let trash_dir = dir.path().join(".sift-trash");
        std::fs::create_dir_all(&dl).unwrap();
        std::fs::create_dir_all(&kept).unwrap();
        std::fs::create_dir_all(&trash_dir).unwrap();

        let source = dl.join("12 - vieux nom.wav");
        let dest = kept.join("Artiste - Titre (Club Mix).aif");
        let trashed = trash_dir.join("12 - vieux nom.wav");
        let (source_s, dest_s, trashed_s) = (
            source.to_str().unwrap().to_string(),
            dest.to_str().unwrap().to_string(),
            trashed.to_str().unwrap().to_string(),
        );

        // État du disque APRÈS `execute_file` d'un rangement non conformant : le converti est à
        // `dest`, l'original (taille différente) est à la corbeille.
        std::fs::write(&dest, vec![b'd'; 4096]).unwrap();
        std::fs::write(&trashed, vec![b's'; 64]).unwrap();

        conn.execute(
            "INSERT INTO tracks(path, filename, size_bytes, mtime, status)
             VALUES(?1, '12 - vieux nom.wav', 1, 1, 'pending')",
            params![source_s],
        )
        .unwrap();
        let track_id = conn.last_insert_rowid();

        let plan = commit_plan_stub(track_id, "b-revert", &source_s, &dest_s);
        let log = vec![
            FsLog {
                kind: "convert",
                from: source_s.clone(),
                to: dest_s.clone(),
                meta: None,
            },
            FsLog {
                kind: "trash",
                from: source_s.clone(),
                to: trashed_s.clone(),
                meta: None,
            },
        ];
        commit_file(&conn, &plan, log, None, None).expect("commit_file");

        let filed_path: String = conn
            .query_row(
                "SELECT path FROM tracks WHERE id=?1",
                params![track_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            filed_path, dest_s,
            "préalable : le rangement repointe `path`"
        );

        crate::actions::revert_batch(&conn, "b-revert").expect("revert_batch");

        let restored_meta =
            std::fs::metadata(&source).expect("la source est revenue sur le disque");
        let (path, filename, size, mtime, status): (String, String, i64, i64, String) = conn
            .query_row(
                "SELECT path, filename, size_bytes, mtime, status FROM tracks WHERE id=?1",
                params![track_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .unwrap();
        assert_eq!(status, "pending");
        assert_eq!(
            path, source_s,
            "l'annulation doit ramener `path` sur la source, la ou revert_one_fs a remis le fichier"
        );
        assert_eq!(filename, "12 - vieux nom.wav");
        assert_eq!(
            size, 64,
            "size_bytes doit redécrire la source restaurée, pas le converti supprimé"
        );
        assert_eq!(mtime, crate::scanner::mtime_secs(&restored_meta));
    }

    /// Condition (b) : un rangement EN PLACE (`FILE_IN_PLACE`) laisse le fichier dans un dossier
    /// SURVEILLÉ. Si `path` suit le fichier mais pas `size_bytes`/`mtime`, le passage suivant du
    /// watcher trouve à ce chemin une ligne dont la taille/date sont celles d'AVANT l'écriture des
    /// tags, prend la branche `Some(_)` de `scanner::upsert_file` et repasse la piste en `pending`
    /// en effaçant `analyzed_at`/`fingerprint`/`report_json` : la piste rangée se dé-range seule.
    #[test]
    fn un_rescan_du_fichier_range_en_place_ne_le_derange_pas() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let watched = dir.path().join("Watched");
        std::fs::create_dir_all(&watched).unwrap();
        conn.execute(
            "INSERT INTO sources(path) VALUES(?1)",
            params![watched.to_str().unwrap()],
        )
        .unwrap();
        let source_id = conn.last_insert_rowid();

        let source = watched.join("12 - vieux nom.aif");
        let dest = watched.join("Artiste - Titre (Club Mix).aif");
        let (source_s, dest_s) = (
            source.to_str().unwrap().to_string(),
            dest.to_str().unwrap().to_string(),
        );

        // Ce que le scanner a enregistré : le fichier tel qu'il était AVANT le rangement.
        std::fs::write(&source, vec![b'x'; 2048]).unwrap();
        let scanned = std::fs::metadata(&source).unwrap();
        conn.execute(
            "INSERT INTO tracks(path, filename, size_bytes, mtime, source_id, status, analyzed_at, fingerprint, fingerprint_ver, report_json)
             VALUES(?1, '12 - vieux nom.aif', ?2, ?3, ?4, 'pending', '2026-07-31T00:00:00Z', 'fp', ?5, '{}')",
            params![
                source_s,
                scanned.len() as i64,
                crate::scanner::mtime_secs(&scanned),
                source_id,
                crate::fingerprint::FINGERPRINT_CACHE_VERSION
            ],
        )
        .unwrap();
        let track_id = conn.last_insert_rowid();

        // `execute_file` conformant : écriture des tags EN PLACE (la taille et la date changent),
        // puis renommage dans le MÊME dossier surveillé.
        std::fs::write(&source, vec![b'x'; 3072]).unwrap();
        std::fs::rename(&source, &dest).unwrap();

        let plan = commit_plan_stub(track_id, "b-inplace", &source_s, &dest_s);
        let log = vec![FsLog {
            kind: "move",
            from: source_s.clone(),
            to: dest_s.clone(),
            meta: None,
        }];
        commit_file(&conn, &plan, log, None, None).expect("commit_file");

        let filed_path: String = conn
            .query_row(
                "SELECT path FROM tracks WHERE id=?1",
                params![track_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            filed_path, dest_s,
            "préalable : le rangement repointe `path`"
        );

        // Passage suivant du watcher sur le fichier tel qu'il est MAINTENANT sur le disque.
        let now = std::fs::metadata(&dest).unwrap();
        let seen = crate::scanner::DiskFile {
            path: dest_s.clone(),
            filename: "Artiste - Titre (Club Mix).aif".to_string(),
            size_bytes: now.len() as i64,
            mtime: crate::scanner::mtime_secs(&now),
        };
        crate::scanner::upsert_file(&conn, source_id, &seen).unwrap();

        // `fingerprint` est relue par `fingerprint::cached`, comme en production : garder la valeur
        // en effaçant sa version rendrait le cache muet — un décodage audio complet au prochain
        // dédoublonnage, sans que rien ne le signale (issue #39).
        let (status, analyzed_at, fingerprint, report): (
            String,
            Option<String>,
            Option<String>,
            Option<String>,
        ) = conn
            .query_row(
                "SELECT status, analyzed_at, fingerprint, report_json, fingerprint_ver \
                 FROM tracks WHERE id=?1",
                params![track_id],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        crate::fingerprint::cached(r.get(2)?, r.get(4)?),
                        r.get(3)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(
            status, "filed",
            "un rescan du fichier range ne doit pas le repasser en pending"
        );
        assert!(analyzed_at.is_some(), "analyzed_at ne doit pas être effacé");
        assert!(fingerprint.is_some(), "fingerprint ne doit pas être effacé");
        assert!(report.is_some(), "report_json ne doit pas être effacé");

        let rows: i64 = conn
            .query_row("SELECT count(*) FROM tracks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 1, "aucune ligne parasite ne doit apparaître");
    }

    /// Condition (c), cas rattrapable : entre le déplacement et la transaction, le watcher a inséré
    /// une ligne `pending` pour le fichier à sa destination. C'est exactement la ligne parasite du
    /// fichier que nous sommes en train de ranger — elle est évincée, le rangement passe.
    #[test]
    fn commit_file_evince_la_ligne_pending_concurrente_a_la_destination() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("KEPT")).unwrap();
        let source = dir.path().join("12 - vieux nom.aif");
        let dest = dir
            .path()
            .join("KEPT")
            .join("Artiste - Titre (Club Mix).aif");
        let (source_s, dest_s) = (
            source.to_str().unwrap().to_string(),
            dest.to_str().unwrap().to_string(),
        );
        std::fs::write(&dest, vec![b'x'; 512]).unwrap(); // le fichier est déjà déplacé

        conn.execute(
            "INSERT INTO tracks(path, status) VALUES(?1, 'pending')",
            params![source_s],
        )
        .unwrap();
        let track_id = conn.last_insert_rowid();
        // La ligne que le watcher vient de créer pour le fichier arrivé à destination.
        conn.execute(
            "INSERT INTO tracks(path, filename, status) VALUES(?1, 'Artiste - Titre (Club Mix).aif', 'pending')",
            params![dest_s],
        )
        .unwrap();

        let plan = commit_plan_stub(track_id, "b-collision", &source_s, &dest_s);
        let log = vec![FsLog {
            kind: "move",
            from: source_s.clone(),
            to: dest_s.clone(),
            meta: None,
        }];
        commit_file(&conn, &plan, log, None, None)
            .expect("une ligne pending concurrente ne doit pas faire échouer le rangement");

        let (path, status): (String, String) = conn
            .query_row(
                "SELECT path, status FROM tracks WHERE id=?1",
                params![track_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(path, dest_s);
        assert_eq!(status, "filed");
        let rows: i64 = conn
            .query_row("SELECT count(*) FROM tracks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 1, "la ligne pending concurrente doit avoir disparu");
    }

    /// Condition (c), cas non rattrapable : une piste DÉJÀ RANGÉE occupe ce chemin. Ce n'est plus
    /// une ligne parasite du watcher mais un vrai conflit métier — refus explicite, pas de
    /// suppression silencieuse, pas de suffixe automatique. `rollback_fs` remet le fichier à sa
    /// source.
    #[test]
    fn commit_file_refuse_une_destination_occupee_par_une_piste_rangee() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("KEPT")).unwrap();
        let source = dir.path().join("12 - vieux nom.aif");
        let dest = dir
            .path()
            .join("KEPT")
            .join("Artiste - Titre (Club Mix).aif");
        let (source_s, dest_s) = (
            source.to_str().unwrap().to_string(),
            dest.to_str().unwrap().to_string(),
        );
        std::fs::write(&dest, vec![b'x'; 512]).unwrap(); // le fichier est déjà déplacé

        conn.execute(
            "INSERT INTO tracks(path, status) VALUES(?1, 'pending')",
            params![source_s],
        )
        .unwrap();
        let track_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO tracks(path, status, folder) VALUES(?1, 'filed', 'House')",
            params![dest_s],
        )
        .unwrap();

        let plan = commit_plan_stub(track_id, "b-occupee", &source_s, &dest_s);
        let log = vec![FsLog {
            kind: "move",
            from: source_s.clone(),
            to: dest_s.clone(),
            meta: None,
        }];
        let err = commit_file(&conn, &plan, log, None, None)
            .expect_err("une piste déjà rangée à ce chemin est un vrai conflit");
        assert_eq!(err, FilingError::DestOccupied(dest_s.clone()));

        // Le rangement a été défait : le fichier est revenu à sa source, la piste reste pending.
        assert!(source.exists(), "rollback_fs doit ramener le fichier");
        assert!(!dest.exists());
        let (path, status): (String, String) = conn
            .query_row(
                "SELECT path, status FROM tracks WHERE id=?1",
                params![track_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(path, source_s);
        assert_eq!(status, "pending");
        let occupant: String = conn
            .query_row(
                "SELECT status FROM tracks WHERE path=?1",
                params![dest_s],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(occupant, "filed", "l'occupant ne doit pas être supprimé");
    }

    #[test]
    fn commit_file_detects_masterdb_repair_with_correct_action_id() {
        let conn = db();
        let tmp = tempfile::tempdir().unwrap();

        let pioneer_dir = tmp.path().join("pioneer");
        std::fs::create_dir_all(&pioneer_dir).unwrap();
        std::fs::copy(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/rekordbox_master.db"
            ),
            pioneer_dir.join("master.db"),
        )
        .unwrap();
        crate::actions::set_pioneer_dir_override_for_test(pioneer_dir.clone());
        let xml_path = pioneer_dir.join("masterPlaylists6.xml");
        std::fs::write(&xml_path, b"<DJ_PLAYLISTS/>").unwrap();
        crate::settings::set(
            &conn,
            crate::settings::REKORDBOX_XML_PATH,
            xml_path.to_str().unwrap(),
        )
        .unwrap();

        conn.execute(
            "INSERT INTO tracks(path, status) VALUES('irrelevant', 'pending')",
            [],
        )
        .unwrap();
        let track_id = conn.last_insert_rowid();

        let plan = FilePlan {
            track_id,
            batch_id: "b1".to_string(),
            source: "irrelevant-source".to_string(),
            dest: "irrelevant-dest".to_string(),
            conformant: false,
            target: Target::Mp3320,
            canonical: Canonical {
                artist: "A".to_string(),
                title: "T".to_string(),
                version: None,
                label: None,
                confidence: naming::Confidence::Green,
            },
            bin_rel: "House".to_string(),
            extras: TagExtras {
                label: None,
                year: None,
                genres: vec![],
                cover_path: None,
            },
        };
        let log = vec![FsLog {
            kind: "move",
            from: "D:/FIXTURE/track1.mp3".to_string(),
            to: "D:/FIXTURE/renamed/track1.flac".to_string(),
            meta: None,
        }];

        commit_file(
            &conn,
            &plan,
            log,
            None,
            actions::resolve_masterdb_index_if_linked(&conn).as_ref(),
        )
        .expect("commit_file");

        let action_id: i64 = conn
            .query_row(
                "SELECT id FROM actions WHERE type='move' AND from_path='D:/FIXTURE/track1.mp3'",
                [],
                |r| r.get(0),
            )
            .expect("move action row exists");

        let (repair_action_id, repair_track_id, status): (i64, String, String) = conn
            .query_row(
                "SELECT action_id, track_id, status FROM rekordbox_masterdb_repairs WHERE from_path='D:/FIXTURE/track1.mp3'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .expect("repair row created");
        assert_eq!(
            repair_action_id, action_id,
            "the repair row must reference the SAME action_id commit_file just created for this row"
        );
        assert_eq!(repair_track_id, "40000001");
        assert_eq!(status, "pending");
    }

    #[test]
    fn commit_file_conformant_detects_masterdb_artwork_sync_only_when_cover_changes() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("lib");
        std::fs::create_dir_all(root.join("House")).unwrap();
        let Some((id, src)) = seed_track(&conn, dir.path(), "real_320.mp3", "src.mp3") else {
            eprintln!("skip: no fixture");
            return;
        };

        let pioneer_dir = dir.path().join("pioneer");
        let xml_path = seed_pioneer_dir_with_fixture(&pioneer_dir);
        patch_fixture_folder_path(&pioneer_dir, src.to_str().unwrap());
        crate::settings::set(
            &conn,
            crate::settings::REKORDBOX_XML_PATH,
            xml_path.to_str().unwrap(),
        )
        .unwrap();

        // No cover_path set on this track's metadata row — commit must NOT create an artwork
        // sync candidate, only a metadata one (already covered by the sibling test).
        let res = file_track(
            &conn,
            &root,
            "{artist} - {title}",
            id,
            "House",
            None,
            Some(Canonical {
                artist: "Larry Heard".into(),
                title: "Can You Feel It".into(),
                version: None,
                label: None,
                confidence: crate::naming::Confidence::Green,
            }),
            false,
        )
        .unwrap();
        let _ = res;

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM rekordbox_masterdb_artwork_syncs WHERE track_id=?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 0,
            "no cover_path on this track — must not create an artwork sync candidate"
        );
    }

    #[test]
    fn commit_file_conformant_detects_masterdb_artwork_sync_when_cover_present() {
        let conn = db();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("lib");
        std::fs::create_dir_all(root.join("House")).unwrap();
        let Some((id, src)) = seed_track(&conn, dir.path(), "real_320.mp3", "src.mp3") else {
            eprintln!("skip: no fixture");
            return;
        };
        conn.execute(
            "INSERT INTO metadata(track_id, cover_path) VALUES (?1, '/cache/covers/999.jpg')
             ON CONFLICT(track_id) DO UPDATE SET cover_path=excluded.cover_path",
            params![id],
        )
        .unwrap();

        let pioneer_dir = dir.path().join("pioneer");
        let xml_path = seed_pioneer_dir_with_fixture(&pioneer_dir);
        patch_fixture_folder_path(&pioneer_dir, src.to_str().unwrap());
        crate::settings::set(
            &conn,
            crate::settings::REKORDBOX_XML_PATH,
            xml_path.to_str().unwrap(),
        )
        .unwrap();

        let res = file_track(
            &conn,
            &root,
            "{artist} - {title}",
            id,
            "House",
            None,
            Some(Canonical {
                artist: "Larry Heard".into(),
                title: "Can You Feel It".into(),
                version: None,
                label: None,
                confidence: crate::naming::Confidence::Green,
            }),
            false,
        )
        .unwrap();
        let _ = res;

        let (cover_path, status): (String, String) = conn
            .query_row(
                "SELECT cover_path, status FROM rekordbox_masterdb_artwork_syncs WHERE track_id=?1",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("commit_file must have detected an artwork sync candidate");
        assert_eq!(cover_path, "/cache/covers/999.jpg");
        assert_eq!(status, "pending");
    }

    // Contract tests (Phase 2) — see
    // docs/superpowers/plans/2026-07-13-phase2-ipc-contract-tests.md. No codegen: these parse
    // shared/contracts.ts's source text directly and assert the Rust constant's literal value
    // appears in it. Inline here (not a separate integration test file) because `filing` is a
    // private module — src-tauri/tests/*.rs compiles as an external crate and can't see it.
    const CONTRACTS_TS: &str = include_str!("../../shared/contracts.ts");

    #[test]
    fn file_in_place_constant_matches_contracts_ts() {
        let expected = format!("\"{}\"", FILE_IN_PLACE);
        assert!(
            CONTRACTS_TS.contains(&expected),
            "shared/contracts.ts must contain FILE_IN_PLACE = {expected}"
        );
    }

    #[test]
    fn external_dest_prefix_constant_matches_contracts_ts() {
        let expected = format!("\"{}\"", EXTERNAL_DEST_PREFIX);
        assert!(
            CONTRACTS_TS.contains(&expected),
            "shared/contracts.ts must contain EXTERNAL_DEST_PREFIX = {expected}"
        );
    }

    /// Le modèle par défaut est affiché ET proposé en réinitialisation par l'écran Réglages : le
    /// front doit connaître sa valeur, donc elle traverse l'IPC en littéral recopié — même contrat
    /// de miroir que les sentinelles ci-dessus.
    #[test]
    fn default_template_matches_contracts_ts() {
        let expected = format!("\"{}\"", crate::settings::DEFAULT_TEMPLATE);
        assert!(
            CONTRACTS_TS.contains(&expected),
            "shared/contracts.ts must contain DEFAULT_FILENAME_TEMPLATE = {expected}"
        );
    }

    /// ... et les trois placeholders que ce modèle utilise doivent être ceux que
    /// `render_filename` sait réellement remplacer. Un quatrième ajouté au défaut sans être câblé
    /// ressortirait littéralement dans le nom de fichier.
    #[test]
    fn default_template_placeholders_are_all_rendered() {
        let c = naming::Canonical {
            artist: "A".into(),
            title: "T".into(),
            version: Some("V".into()),
            label: None,
            confidence: naming::Confidence::Green,
        };
        let out = naming::render_filename(crate::settings::DEFAULT_TEMPLATE, &c, "aiff");
        assert!(
            !out.contains('{') && !out.contains('}'),
            "un placeholder du modèle par défaut n'est pas rendu: {out}"
        );
        assert_eq!(out, "A - T (V).aiff");
    }

    /// La seule sentinelle dont la rupture SUPPRIME une ligne de la base : `ipc::analyze_path` la
    /// reconnaît pour appeler `scanner::forget_path`, et `frontend/filing.ts` pour basculer la
    /// fiche en « fichier introuvable ». Elle traversait l'IPC en littéral recopié des deux côtés.
    #[test]
    fn file_gone_constant_matches_contracts_ts() {
        let expected = format!("\"{}\"", crate::analysis::decode::FILE_GONE);
        assert!(
            CONTRACTS_TS.contains(&expected),
            "shared/contracts.ts must contain FILE_GONE = {expected}"
        );
    }

    /// Le message produit par `decode` doit RÉELLEMENT contenir la sentinelle : sans ça, les deux
    /// constantes peuvent rester d'accord entre elles pendant que plus personne ne l'émet.
    #[test]
    fn the_missing_file_message_actually_carries_the_sentinel() {
        let err = crate::analysis::decode::probe("definitely/does/not/exist_ever.flac")
            .expect_err("probing a missing file must fail");
        assert!(
            err.contains(crate::analysis::decode::FILE_GONE),
            "le message de fichier disparu doit porter la sentinelle: {err}"
        );
    }

    /// Mirrors shared/contracts.ts's `BatchResult`. Exhaustive destructure (no `..`): fails to
    /// compile if a field is added/removed/renamed on the Rust struct — the forcing function to
    /// also update contracts.ts. Phase 2 — docs/superpowers/plans/2026-07-13-phase2-ipc-contract-tests.md.
    #[test]
    fn batch_result_shape_matches_contracts_ts() {
        let v = BatchResult {
            filed: 0,
            needs_validation: Vec::new(),
            cancelled: false,
            filed_ids: Vec::new(),
            errors: Vec::new(),
        };
        let BatchResult {
            filed,
            needs_validation,
            cancelled,
            filed_ids,
            errors,
        } = v;
        let _ = (filed, needs_validation, cancelled, filed_ids, errors);
    }
}
