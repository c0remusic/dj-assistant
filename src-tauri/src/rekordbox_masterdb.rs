//! Pure-Rust reader **and** Tier 1 write engine for Rekordbox's SQLCipher-encrypted
//! `master.db` (see `docs/superpowers/specs/2026-07-03-rekordbox-masterdb-sqlcipher-reader-design.md`
//! for the reader, `docs/superpowers/specs/2026-07-06-m8-tier1-write-path-rust-design-v2.md`
//! for the write engine). Reads (`read_rekordbox_masterdb`) are always safe; the write
//! path (`repair_track_path`) is only ever reached through an explicit, user-confirmed
//! IPC call (`ipc_library::rekordbox_masterdb_apply_repairs`) and owns its own
//! guard/backup/verify/rollback safety chain — nothing here writes `master.db` as a side
//! effect of a read.
//!
//! # Approach
//!
//! Rather than reimplementing SQLite's on-disk B-tree format, this module only
//! decrypts each SQLCipher page to plaintext, reassembles a standard
//! (unencrypted) SQLite file buffer in memory, and hands that buffer to
//! `rusqlite` via [`rusqlite::Connection::deserialize_read_exact`] — the rest
//! (B-tree parsing, SQL queries) is then plain SQLite, already proven inside
//! Sift. The decrypted buffer is never written to disk.
//!
//! # SQLCipher v4 parameters (empirically confirmed, not assumed)
//!
//! Confirmed against a real `sqlcipher3` connection (`PRAGMA kdf_iter`,
//! `PRAGMA cipher_kdf_algorithm`, `PRAGMA cipher_hmac_algorithm`) and by a
//! full round-trip decrypt of a synthetic fixture (all pages HMAC-verified,
//! reconstructed buffer readable by stdlib SQLite):
//!
//! - Key derivation: PBKDF2-HMAC-SHA512, 256 000 iterations, 32-byte key.
//! - HMAC key derivation: PBKDF2-HMAC-SHA512, salt XORed with `0x3a`, 2
//!   iterations, 32-byte key.
//! - Page size: 4096 bytes. Reserve: 80 bytes (16-byte IV + 64-byte
//!   HMAC-SHA512 — already a multiple of the AES block size, no extra
//!   padding).
//! - Per-page HMAC covers `ciphertext || iv || page_number` (`page_number`
//!   as little-endian `u32`, 1-indexed), verified *before* decryption.
//! - Page 1 special case: the first 16 bytes on disk are the (unencrypted)
//!   salt; only `page_size - 16 - reserve` bytes of page 1 are ciphertext.
//!
//! Note: the SQLite magic string `"SQLite format 3\0"` lives in that
//! never-encrypted salt region, so it is *not* usable as a correctness
//! check (it "matches" even with a wrong key). Correctness here is
//! established by the per-page HMAC (fail-fast) plus the fact that the
//! reassembled buffer parses as a valid SQLite database.
//!
//! # Status
//!
//! Wired to IPC: `read_rekordbox_masterdb` (via `actions::detect_masterdb_repair_if_linked`,
//! read-only), `repair_track_path` (Tier 1, via `ipc_library::rekordbox_masterdb_apply_repairs`),
//! `dedup_playlist_group` (Tier 2). `sync_track_metadata` (Tier 3, metadata
//! find-or-create) is proven on fixture + a real `master.db` copy
//! (`docs/superpowers/plans/2026-07-09-m8-tier3-metadata-sync-rust.md`) but
//! **not yet wired to IPC or a filing-time hook** — same "engine first"
//! precedent as Tier 1/2, follow-up plan pending.
//!
//! # Real-copy tests run one at a time
//!
//! The 3 `#[ignore]`d `*_round_trips_on_real_masterdb_copy` tests (Tier 1/2/3)
//! all read `SIFT_M8_REAL_COPY_DIR` and mutate the *same* `master.db` file —
//! `cargo test -- --ignored` runs tests in parallel by default, so running
//! more than one of them in the same invocation races on that file and
//! produces spurious failures (discovered 2026-07-09 running all 3 together).
//! Always filter to exactly one: `cargo test --lib -- --exact
//! rekordbox_masterdb::tests::<test_name> --ignored --test-threads=1`.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use aes::Aes256;
use cbc::cipher::block_padding::NoPadding;
use cbc::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use flate2::read::ZlibDecoder;
use hmac::{Hmac, Mac};
use image::ImageEncoder;
use pbkdf2::pbkdf2_hmac;
use rand::rngs::OsRng;
use rand::{Rng, RngCore};
use rusqlite::{Connection, OptionalExtension, Transaction};
use sha2::Sha512;
use std::io::Read;
use sysinfo::System;
use uuid::Uuid;

/// SQLCipher v4 default page size, confirmed via `PRAGMA cipher_page_size`.
const PAGE_SIZE: usize = 4096;
/// IV (16 bytes) + full HMAC-SHA512 digest (64 bytes); already AES-block-aligned.
const RESERVE: usize = 80;
/// SQLCipher v4 default KDF iteration count, confirmed via `PRAGMA kdf_iter`.
const KDF_ITER: u32 = 256_000;
/// SQLCipher v4 default HMAC-key derivation iteration count.
const HMAC_KDF_ITER: u32 = 2;
/// Byte XORed into the salt to derive the HMAC-key salt (SQLCipher convention).
const HMAC_SALT_XOR: u8 = 0x3a;
/// Salt occupies the first 16 bytes of the file (and of page 1 on disk).
const SALT_LEN: usize = 16;

/// `NoPadding` requires every AES-CBC input to be exactly block-aligned (16
/// bytes). The page-body lengths derived from these constants
/// (`PAGE_SIZE - RESERVE` for most pages, `PAGE_SIZE - RESERVE - SALT_LEN`
/// for page 1) are currently aligned only because `RESERVE`/`SALT_LEN`/
/// `PAGE_SIZE` all happen to be multiples of 16 — nothing else enforces
/// that. If a future edit to these constants broke it, `encrypt_masterdb`/
/// `decrypt_page_body` would fail unpredictably instead of failing here,
/// at compile time, with a clear reason.
const _: () = assert!(
    PAGE_SIZE % 16 == 0 && RESERVE % 16 == 0 && SALT_LEN % 16 == 0,
    "PAGE_SIZE/RESERVE/SALT_LEN must all be AES-block-aligned (16 bytes) for NoPadding CBC"
);

type Aes256CbcDec = cbc::Decryptor<Aes256>;
type Aes256CbcEnc = cbc::Encryptor<Aes256>;
type HmacSha512 = Hmac<Sha512>;

/// Deobfuscated Rekordbox `master.db` passphrase, base85(RFC1924)+XOR+zlib
/// obfuscated in Rekordbox's own source (mirrored from the public,
/// documented constants in `pyrekordbox`: `db6/database.py`'s `BLOB` and
/// `utils.py`'s `BLOB_KEY`). This is a static, publicly known constant, not a
/// per-installation secret.
const BLOB: &str = "PN_Pq^*N>(JYe*u^8;Yg76HuZ<mR13S?=>)b9;DpoTXV(6ItkU`}8*m6tx_I{Solh_N#dfe{v=";
const BLOB_KEY: &[u8] = b"657f48f84c437cc1";

/// Why reading `master.db` failed. Kept in the same style as other Sift
/// internal error enums (`FilingError`, `EncodeError`): `Debug + Clone +
/// PartialEq` + manual `Display`, converted to `String` at any future IPC
/// boundary rather than derived `Serialize` directly (no IPC command exists
/// for this module yet — that wiring is explicit out-of-scope for now).
#[derive(Debug, Clone, PartialEq)]
pub enum MasterDbError {
    /// Could not read the file from disk.
    Io(String),
    /// The file is smaller than one SQLCipher page — not a valid `master.db`.
    FileTooShort,
    /// The file's size is not an exact multiple of `PAGE_SIZE` — a truncated
    /// or partially-written file. Refuses to silently drop the trailing
    /// partial page (and whatever rows might live on it) via integer
    /// division.
    TruncatedFile {
        /// Total size on disk, in bytes.
        len: usize,
    },
    /// Deobfuscating the static passphrase constant failed (base85/zlib).
    KeyDeobfuscation(String),
    /// A page's HMAC did not match — refuses to trust its decrypted content.
    HmacMismatch {
        /// 1-indexed page number that failed verification.
        page: u32,
    },
    /// The reassembled plaintext buffer was rejected by SQLite itself.
    Sqlite(String),
    /// AES-CBC decryption itself failed (e.g. ciphertext not block-aligned).
    /// Distinct from `HmacMismatch`: this can only happen if the HMAC check
    /// already passed but the page geometry is still malformed — should not
    /// occur for the confirmed page size/reserve, kept as a fail-fast guard
    /// rather than a silent panic.
    Decrypt {
        /// 1-indexed page number that failed to decrypt.
        page: u32,
    },
    /// A write was refused because Rekordbox is currently running — write
    /// must never proceed while Rekordbox might also be touching the file.
    RekordboxRunning,
    /// `agentRegistry` has no `localUpdateCount` row — the file's shape
    /// doesn't match what Tier 1 assumes, refuse rather than guess a USN.
    RegistryRowMissing,
    /// No `djmdContent` row with the given `ID` — nothing to repair.
    TrackNotFound {
        /// The `djmdContent.ID` that was not found.
        track_id: String,
    },
    /// The write succeeded but re-reading the file afterwards didn't show
    /// the expected value — backup was restored automatically.
    WriteVerificationFailedRolledBack(String),
    /// The write succeeded, re-reading it failed verification, AND
    /// restoring the backup also failed — the live file may now be in a
    /// bad state and needs manual attention.
    WriteVerificationFailedRollbackFailed(String),
    /// `dedup_playlist_group` was called with a group that has nothing to
    /// remove — the caller should have filtered this out via
    /// `detect_playlist_duplicates` first.
    #[allow(dead_code)]
    NoDuplicatesToRemove,
    /// A `djmdSongPlaylist.ID` from `PlaylistDuplicateGroup::remove` no
    /// longer matched any row at delete time (already removed by something
    /// else since detection ran).
    #[allow(dead_code)]
    SongPlaylistEntryNotFound {
        /// The `djmdSongPlaylist.ID` that was not found.
        song_playlist_id: String,
    },
    /// `find_or_create_named_row` was called with a table name outside the
    /// 3 whitelisted FK tables — programmer error, never user input (the
    /// table name is always a Sift-internal constant), fail-fast rather
    /// than build SQL from an unchecked string.
    #[allow(dead_code)]
    UnknownFkTable {
        /// The rejected table name.
        table: String,
    },
    /// Could not find a free 32-bit ID after repeated random attempts —
    /// astronomically unlikely for the real table sizes involved, kept as a
    /// fail-fast rather than an infinite loop.
    #[allow(dead_code)]
    IdGenerationExhausted {
        /// The FK table for which ID generation failed.
        table: String,
    },
    /// `djmdContent.ImagePath` est NULL/vide pour cette piste — aucun
    /// mécanisme de création connu (non testé au spike 8), refuser plutôt
    /// que deviner un comportement Rekordbox non observé.
    #[allow(dead_code)]
    NoArtworkPath {
        /// La piste sans pochette.
        track_id: String,
    },
    /// `ImagePath` pointe vers un chemin dont une des 3 variantes
    /// (pleine/moyenne/miniature) n'existe pas sur disque — refuse plutôt
    /// que de deviner les dimensions d'un fichier absent.
    #[allow(dead_code)]
    ArtworkVariantMissing {
        /// Le chemin résolu manquant.
        path: String,
    },
    /// L'écriture des fichiers artwork a réussi mais la relecture ne montre
    /// pas les dimensions attendues — backup restauré automatiquement.
    #[allow(dead_code)]
    ArtworkWriteVerificationFailedRolledBack(String),
    /// Idem, mais la restauration du backup a aussi échoué — les fichiers
    /// artwork live peuvent être dans un état incohérent.
    #[allow(dead_code)]
    ArtworkWriteVerificationFailedRollbackFailed(String),
    /// L'écriture d'un des 3 fichiers artwork (full/medium/small) a échoué en
    /// cours de boucle (écriture non-atomique multi-fichiers, contrairement au
    /// tmp+rename mono-fichier de master.db) ET la restauration du backup a
    /// aussi échoué — les fichiers artwork live peuvent être dans un état
    /// incohérent (certaines variantes déjà réécrites, d'autres non).
    #[allow(dead_code)]
    ArtworkWriteFailedRollbackFailed(String),
    /// `djmdContent.ImagePath` contient un composant `..` ou un préfixe de
    /// lecteur — refuse plutôt que de résoudre un chemin hors de
    /// `pioneer_dir/share` (containment, même logique que
    /// `library::safe_join`).
    #[allow(dead_code)]
    ArtworkPathEscapesRoot {
        /// L'`ImagePath` brut refusé.
        path: String,
    },
}

impl std::fmt::Display for MasterDbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MasterDbError::Io(m) => write!(f, "io: {m}"),
            MasterDbError::FileTooShort => write!(f, "file too short to be a master.db"),
            MasterDbError::TruncatedFile { len } => write!(
                f,
                "file size ({len} bytes) is not a multiple of the {PAGE_SIZE}-byte page size — truncated or corrupted"
            ),
            MasterDbError::KeyDeobfuscation(m) => write!(f, "key deobfuscation: {m}"),
            MasterDbError::HmacMismatch { page } => {
                write!(f, "HMAC mismatch on page {page} — refusing to trust decrypted content")
            }
            MasterDbError::Sqlite(m) => write!(f, "sqlite: {m}"),
            MasterDbError::Decrypt { page } => write!(f, "AES-CBC decrypt failed on page {page}"),
            MasterDbError::RekordboxRunning => {
                write!(f, "refusing to write: Rekordbox is currently running")
            }
            MasterDbError::RegistryRowMissing => {
                write!(f, "agentRegistry has no localUpdateCount row")
            }
            MasterDbError::TrackNotFound { track_id } => {
                write!(f, "no djmdContent row with ID {track_id}")
            }
            MasterDbError::WriteVerificationFailedRolledBack(m) => {
                write!(f, "write verification failed, backup restored: {m}")
            }
            MasterDbError::WriteVerificationFailedRollbackFailed(m) => {
                write!(f, "write verification failed AND rollback failed — manual attention needed: {m}")
            }
            MasterDbError::NoDuplicatesToRemove => {
                write!(f, "dedup_playlist_group called with an empty remove list")
            }
            MasterDbError::SongPlaylistEntryNotFound { song_playlist_id } => {
                write!(f, "no djmdSongPlaylist row with ID {song_playlist_id}")
            }
            MasterDbError::UnknownFkTable { table } => {
                write!(f, "unknown FK table for find-or-create: {table}")
            }
            MasterDbError::IdGenerationExhausted { table } => {
                write!(f, "could not generate a free ID for table {table}")
            }
            MasterDbError::NoArtworkPath { track_id } => {
                write!(f, "djmdContent row {track_id} has no ImagePath")
            }
            MasterDbError::ArtworkVariantMissing { path } => {
                write!(f, "expected artwork variant file missing: {path}")
            }
            MasterDbError::ArtworkWriteVerificationFailedRolledBack(m) => {
                write!(f, "artwork write verification failed, backup restored: {m}")
            }
            MasterDbError::ArtworkWriteVerificationFailedRollbackFailed(m) => {
                write!(
                    f,
                    "artwork write verification failed AND rollback failed — manual attention needed: {m}"
                )
            }
            MasterDbError::ArtworkWriteFailedRollbackFailed(m) => {
                write!(
                    f,
                    "artwork write failed AND rollback failed — manual attention needed: {m}"
                )
            }
            MasterDbError::ArtworkPathEscapesRoot { path } => {
                write!(f, "ImagePath escapes the share root, refusing to resolve: {path}")
            }
        }
    }
}

impl std::error::Error for MasterDbError {}

/// One entry of the path→TrackID index read from `master.db`. Field names
/// match the `chemin → TrackID` index consumed by the M7 XML module
/// (`2026-07-03-m7-rekordbox-xml-export-design.md`) so the two sources are
/// interchangeable there — this module does not itself wire into that one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RekordboxTrack {
    /// Rekordbox `djmdContent.ID`.
    pub track_id: String,
    /// Rekordbox `djmdContent.FolderPath` (full file path as Rekordbox knows it).
    pub folder_path: String,
}

/// Path→TrackID index read from a Rekordbox `master.db`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RekordboxIndex {
    /// All tracks found in `djmdContent`.
    pub tracks: Vec<RekordboxTrack>,
}

/// One Tier 1 path-repair operation: the 3 `djmdContent` path columns that
/// must move together (`FolderPath`, `FileNameL`, `FileNameS` — confirmed by
/// the M8 spike to always change as a set, never `FolderPath` alone).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathRepair {
    /// Rekordbox `djmdContent.ID` of the row to repair.
    pub track_id: String,
    /// New `FolderPath`.
    pub new_folder_path: String,
    /// New `FileNameL`.
    pub new_file_name_l: String,
    /// New `FileNameS`.
    pub new_file_name_s: String,
}

/// One M8 Tier 3 metadata sync operation: mirrors exactly the fields
/// `tagging::write_tags_full` writes to the audio file (artist/title/
/// label/year/genre) — cover and album are deliberately absent, neither is
/// ever written by Sift's own tagging path. Fields left `None` are not
/// touched, same "None = leave alone" convention as `write_tags_full`
/// itself.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MetadataSync {
    /// Rekordbox `djmdContent.ID` of the row to sync.
    pub track_id: String,
    /// New artist name — find-or-create on `djmdArtist`, repoints `ArtistID`.
    pub artist: Option<String>,
    /// New title — direct write to `djmdContent.Title`, no FK involved.
    pub title: Option<String>,
    /// New release year — direct write to `djmdContent.ReleaseYear`, no FK.
    pub year: Option<i64>,
    /// New genre (already joined "A; B" the way Sift writes a single ID3
    /// Genre field) — find-or-create on `djmdGenre`, repoints `GenreID`.
    pub genre: Option<String>,
    /// New label (Sift's ID3 Publisher/TPUB field) — find-or-create on
    /// `djmdLabel`, repoints `LabelID`.
    pub label: Option<String>,
}

/// One `djmdSongPlaylist` row involved in a duplicate group — either the
/// occurrence being kept or one being removed.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaylistDuplicateEntry {
    /// Rekordbox `djmdSongPlaylist.ID` of this row.
    pub song_playlist_id: String,
    /// Rekordbox `djmdSongPlaylist.TrackNo` of this row.
    pub track_no: i64,
}

/// A set of `djmdSongPlaylist` rows in the same playlist that reference the
/// same track more than once. `keep` is the occurrence with the lowest
/// `TrackNo` (kept untouched by `dedup_playlist_group`); `remove` is every
/// other occurrence.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaylistDuplicateGroup {
    /// Rekordbox `djmdPlaylist.ID` the duplicated entries belong to.
    pub playlist_id: String,
    /// Rekordbox `djmdContent.ID` that appears more than once in this playlist.
    pub content_id: String,
    /// The occurrence that survives (lowest `TrackNo`).
    pub keep: PlaylistDuplicateEntry,
    /// Every other occurrence — these are what `dedup_playlist_group` deletes.
    pub remove: Vec<PlaylistDuplicateEntry>,
}

/// Scans `djmdSongPlaylist` for `(PlaylistID, ContentID)` pairs that appear
/// more than once — the same track added twice (or more) to the same
/// playlist. Read-only, mirroring `read_rekordbox_masterdb`'s shape (decrypt
/// → deserialize → query, no write).
#[allow(dead_code)]
pub fn detect_playlist_duplicates(
    path: &Path,
) -> Result<Vec<PlaylistDuplicateGroup>, MasterDbError> {
    let plaintext = read_and_decrypt_cached(path)?;

    let mut conn =
        Connection::open_in_memory().map_err(|e| MasterDbError::Sqlite(e.to_string()))?;
    let len = plaintext.len();
    conn.deserialize_read_exact(rusqlite::MAIN_DB, Cursor::new(plaintext), len, true)
        .map_err(|e| MasterDbError::Sqlite(e.to_string()))?;

    let mut stmt = conn
        .prepare("SELECT ID, PlaylistID, ContentID, TrackNo FROM djmdSongPlaylist ORDER BY PlaylistID, ContentID, TrackNo")
        .map_err(|e| MasterDbError::Sqlite(e.to_string()))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .map_err(|e| MasterDbError::Sqlite(e.to_string()))?;

    let mut all = Vec::new();
    for row in rows {
        all.push(row.map_err(|e| MasterDbError::Sqlite(e.to_string()))?);
    }

    // Rows are sorted by (PlaylistID, ContentID, TrackNo), so duplicates of
    // the same (PlaylistID, ContentID) pair are always contiguous — a single
    // linear scan finds every group without a HashMap.
    let mut groups: Vec<PlaylistDuplicateGroup> = Vec::new();
    let mut i = 0;
    while i < all.len() {
        let (keep_id, playlist_id, content_id, keep_track_no) = &all[i];
        let mut j = i + 1;
        let mut remove = Vec::new();
        while j < all.len() && &all[j].1 == playlist_id && &all[j].2 == content_id {
            remove.push(PlaylistDuplicateEntry {
                song_playlist_id: all[j].0.clone(),
                track_no: all[j].3,
            });
            j += 1;
        }
        if !remove.is_empty() {
            groups.push(PlaylistDuplicateGroup {
                playlist_id: playlist_id.clone(),
                content_id: content_id.clone(),
                keep: PlaylistDuplicateEntry {
                    song_playlist_id: keep_id.clone(),
                    track_no: *keep_track_no,
                },
                remove,
            });
        }
        i = j;
    }
    Ok(groups)
}

/// Reverses Rekordbox's own obfuscation of the static `master.db` passphrase:
/// base85 (RFC1924 alphabet, same as CPython's `base64.b85decode`) decode →
/// XOR with the repeating key → zlib decompress → UTF-8. Order is the exact
/// inverse of `pyrekordbox.utils.obfuscate`.
fn deobfuscate_key() -> Result<String, MasterDbError> {
    let decoded =
        base85::decode(BLOB).map_err(|e| MasterDbError::KeyDeobfuscation(e.to_string()))?;
    let xored: Vec<u8> = decoded
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ BLOB_KEY[i % BLOB_KEY.len()])
        .collect();
    let mut decompressor = ZlibDecoder::new(Cursor::new(xored));
    let mut out = String::new();
    decompressor
        .read_to_string(&mut out)
        .map_err(|e| MasterDbError::KeyDeobfuscation(e.to_string()))?;
    Ok(out)
}

/// Derives the AES key and the separate HMAC key from the passphrase and the
/// file's salt, per SQLCipher v4's two-stage PBKDF2 scheme.
fn derive_keys(passphrase: &str, salt: &[u8; SALT_LEN]) -> ([u8; 32], [u8; 32]) {
    let mut key = [0u8; 32];
    pbkdf2_hmac::<Sha512>(passphrase.as_bytes(), salt, KDF_ITER, &mut key);

    let hmac_salt: Vec<u8> = salt.iter().map(|b| b ^ HMAC_SALT_XOR).collect();
    let mut hmac_key = [0u8; 32];
    pbkdf2_hmac::<Sha512>(&key, &hmac_salt, HMAC_KDF_ITER, &mut hmac_key);

    (key, hmac_key)
}

/// Verifies and decrypts a single SQLCipher page. `raw_page` is exactly
/// `PAGE_SIZE` bytes: for page 1, this is the *whole* on-disk page (salt
/// included) — the salt handling is done by the caller, this function only
/// verifies/decrypts the ciphertext + reserve region.
///
/// Returns the decrypted content, `PAGE_SIZE - RESERVE` bytes for pages
/// after the salt has been stripped by the caller.
fn decrypt_page_body(
    page_no: u32,
    ciphertext: &[u8],
    iv: &[u8; 16],
    stored_hmac: &[u8],
    key: &[u8; 32],
    hmac_key: &[u8; 32],
) -> Result<Vec<u8>, MasterDbError> {
    let mut mac =
        <HmacSha512 as Mac>::new_from_slice(hmac_key).expect("HMAC-SHA512 accepts any key length");
    mac.update(ciphertext);
    mac.update(iv);
    mac.update(&page_no.to_le_bytes());
    mac.verify_slice(stored_hmac)
        .map_err(|_| MasterDbError::HmacMismatch { page: page_no })?;

    // SQLCipher pages carry no PKCS7 padding (the ciphertext region is
    // already a fixed, block-aligned size) — `NoPadding` just decrypts every
    // block in place without trying to strip/validate a padding tail.
    let decryptor = Aes256CbcDec::new(key.into(), iv.into());
    let plain = decryptor
        .decrypt_padded_vec_mut::<NoPadding>(ciphertext)
        .map_err(|_| MasterDbError::Decrypt { page: page_no })?;
    Ok(plain)
}

/// Cached result of the last `read_and_decrypt_cached` call — keyed by path +
/// mtime + len so a write to `master.db` (which always changes at least one
/// of those) self-invalidates it, no explicit invalidation call needed.
struct DecryptCache {
    path: PathBuf,
    mtime: std::time::SystemTime,
    len: u64,
    plaintext: Vec<u8>,
}

static DECRYPT_CACHE: std::sync::OnceLock<std::sync::Mutex<Option<DecryptCache>>> =
    std::sync::OnceLock::new();

/// Reads and decrypts `master.db`, reusing the last decrypted plaintext when the file's
/// mtime+len haven't changed since. The Rekordbox integration page (`rekordbox-view.ts`)
/// calls `read_rekordbox_masterdb`/`read_playlist_names`/`detect_playlist_duplicates`
/// independently after every micro-action (checkbox toggle, dismiss, apply) — before this
/// cache, each of those re-ran the full AES-CBC decrypt + per-page HMAC verify of the whole
/// file on every call, several times per render. Poisoned-mutex/any-error paths fall back to
/// an uncached read rather than failing the caller for a cache-layer problem alone.
fn read_and_decrypt_cached(path: &Path) -> Result<Vec<u8>, MasterDbError> {
    let meta = std::fs::metadata(path).map_err(|e| MasterDbError::Io(e.to_string()))?;
    let mtime = meta
        .modified()
        .map_err(|e| MasterDbError::Io(e.to_string()))?;
    let len = meta.len();

    let cache = DECRYPT_CACHE.get_or_init(|| std::sync::Mutex::new(None));
    if let Ok(guard) = cache.lock() {
        if let Some(c) = guard.as_ref() {
            if c.path == path && c.mtime == mtime && c.len == len {
                return Ok(c.plaintext.clone());
            }
        }
    }

    let raw = std::fs::read(path).map_err(|e| MasterDbError::Io(e.to_string()))?;
    let plaintext = decrypt_masterdb(&raw)?;

    if let Ok(mut guard) = cache.lock() {
        *guard = Some(DecryptCache {
            path: path.to_path_buf(),
            mtime,
            len,
            plaintext: plaintext.clone(),
        });
    }
    Ok(plaintext)
}

/// Decrypts the whole file into a plaintext SQLite buffer.
fn decrypt_masterdb(raw: &[u8]) -> Result<Vec<u8>, MasterDbError> {
    if raw.len() < PAGE_SIZE {
        return Err(MasterDbError::FileTooShort);
    }
    if raw.len() % PAGE_SIZE != 0 {
        // A partial trailing page would otherwise be silently dropped by the
        // integer division below — whatever rows live on it (e.g. tracks in
        // a large djmdContent B-tree) would vanish from the returned index
        // without any signal. Fail fast instead, consistent with this
        // module's strict HMAC verification elsewhere.
        return Err(MasterDbError::TruncatedFile { len: raw.len() });
    }
    let passphrase = deobfuscate_key()?;
    let mut salt = [0u8; SALT_LEN];
    salt.copy_from_slice(&raw[..SALT_LEN]);
    let (key, hmac_key) = derive_keys(&passphrase, &salt);

    let n_pages = raw.len() / PAGE_SIZE;
    let mut out = Vec::with_capacity(raw.len());

    for i in 0..n_pages {
        let page_no = (i + 1) as u32;
        let page = &raw[i * PAGE_SIZE..(i + 1) * PAGE_SIZE];

        let ciphertext_start = if page_no == 1 { SALT_LEN } else { 0 };
        let ciphertext = &page[ciphertext_start..PAGE_SIZE - RESERVE];
        let tail = &page[PAGE_SIZE - RESERVE..];
        let iv: [u8; 16] = tail[..16].try_into().expect("iv is 16 bytes");
        let stored_hmac = &tail[16..16 + 64];

        let mut plain = decrypt_page_body(page_no, ciphertext, &iv, stored_hmac, &key, &hmac_key)?;

        if page_no == 1 {
            // Bytes 18/19 of a standard SQLite header ("file format
            // write/read version") on a *real* Rekordbox master.db read 2/2
            // (WAL), and stay that way even once the companion `-wal`/`-shm`
            // files are gone after a clean Rekordbox shutdown (confirmed
            // empirically: closing Rekordbox removes `master.db-wal`/`-shm`
            // entirely, but master.db's own header byte is never rewritten
            // back to 1). `rusqlite`'s `deserialize_read_exact`/`sqlite3_deserialize`
            // path uses SQLite's in-memory "memdb" VFS, which has no real
            // file to open a `-wal` sidecar against — so a header that
            // claims WAL mode makes the *first* query against the
            // deserialized connection fail with `SQLITE_CANTOPEN` ("unable
            // to open database file"), even though `sqlite3_deserialize`
            // itself reports success. The small synthetic fixture used by
            // this module's other tests was generated fresh in rollback
            // mode (1/1), which is why this never showed up before testing
            // against a real copy (see `docs/plan-implementation.md`, M8
            // Tier 1 status). Forcing 1/1 here is safe for both reads and
            // writes: by the time Sift ever sees this buffer, any real WAL
            // content has already been checkpointed into the page data
            // itself (that's what "Rekordbox is closed" / "no `-wal` file
            // present" guarantees, enforced by `is_rekordbox_running` before
            // any write) — declaring rollback mode doesn't drop data, it
            // just stops SQLite expecting a WAL sidecar that doesn't apply
            // to an in-memory deserialize. On the write side
            // (`encrypt_masterdb`), this buffer is re-serialized from a
            // connection that was never put into WAL mode, so the
            // re-encrypted file legitimately is rollback-mode — Rekordbox
            // itself is free to switch it back to WAL on its next write.
            //
            // Byte 20 ("reserved space per page") must likewise read the
            // *true* reserve (RESERVE = 80), matching what real SQLCipher
            // pages always declare — verified against a genuine page 1
            // (manual PBKDF2+AES-CBC decrypt showed byte 20 = 80, not 0).
            // Declaring it truthfully costs nothing for reads (those
            // trailing bytes were never real SQLite content) and is
            // required for writes: if we declared 0, SQLite would believe
            // the full page is usable and could write real cell content
            // into the last RESERVE bytes, which the re-encryption path
            // (`encrypt_masterdb`) discards as padding — silently dropping
            // data.
            //
            // Offsets below are relative to `plain`, which excludes the
            // 16-byte magic prefix (added separately just below) — so file
            // offset 18 is `plain[2]`, file offset 20 is `plain[4]`.
            // Guarded rather than bare indexing: `plain` is HMAC-verified
            // but its length still depends on AES decrypting to the
            // expected size — a checked write turns any future geometry
            // mismatch into a clear error instead of a panic on malformed
            // input.
            if plain.len() < 5 {
                return Err(MasterDbError::Decrypt { page: page_no });
            }
            plain[2] = 1; // file offset 18: write_version
            plain[3] = 1; // file offset 19: read_version
            match plain.get_mut(4) {
                Some(b) => *b = RESERVE as u8,
                None => return Err(MasterDbError::Decrypt { page: page_no }),
            }
            out.extend_from_slice(b"SQLite format 3\0");
        }
        out.extend_from_slice(&plain);
        // Reassembled pages stay a fixed PAGE_SIZE; the trailing RESERVE
        // bytes are zero padding (the header at offset 20 declares the
        // true reserve, set above — this padding is never real content).
        out.extend(std::iter::repeat(0u8).take(RESERVE));
    }

    Ok(out)
}

/// Inverse of [`decrypt_masterdb`]: takes a plaintext SQLite buffer shaped
/// exactly like that function's output (fixed `PAGE_SIZE` pages, true
/// reserve declared per Task 2's fix) and re-encrypts it as a SQLCipher v4
/// file. Generates a fresh random salt for the output — equivalent to a
/// passphrase-preserving rekey on every full rewrite, which is simpler and
/// just as valid as trying to reuse the original file's salt.
pub(crate) fn encrypt_masterdb(plaintext: &[u8]) -> Result<Vec<u8>, MasterDbError> {
    if plaintext.len() < PAGE_SIZE || plaintext.len() % PAGE_SIZE != 0 {
        return Err(MasterDbError::TruncatedFile {
            len: plaintext.len(),
        });
    }
    let passphrase = deobfuscate_key()?;
    let mut salt = [0u8; SALT_LEN];
    OsRng.fill_bytes(&mut salt);
    let (key, hmac_key) = derive_keys(&passphrase, &salt);

    let n_pages = plaintext.len() / PAGE_SIZE;
    let mut out = Vec::with_capacity(plaintext.len());

    for i in 0..n_pages {
        let page_no = (i + 1) as u32;
        let page = &plaintext[i * PAGE_SIZE..(i + 1) * PAGE_SIZE];

        // Mirrors decrypt_masterdb's page1 special case in reverse: page1's
        // reconstructed plaintext is [16-byte magic][4000-byte body][80-byte
        // pad]; every other page is [4016-byte body][80-byte pad].
        let body = if page_no == 1 {
            &page[SALT_LEN..SALT_LEN + (PAGE_SIZE - RESERVE - SALT_LEN)]
        } else {
            &page[..PAGE_SIZE - RESERVE]
        };

        let mut iv = [0u8; 16];
        OsRng.fill_bytes(&mut iv);

        let encryptor = Aes256CbcEnc::new((&key).into(), (&iv).into());
        let ciphertext = encryptor.encrypt_padded_vec_mut::<NoPadding>(body);

        let mut mac = <HmacSha512 as Mac>::new_from_slice(&hmac_key)
            .expect("HMAC-SHA512 accepts any key length");
        mac.update(&ciphertext);
        mac.update(&iv);
        mac.update(&page_no.to_le_bytes());
        let stored_hmac = mac.finalize().into_bytes();

        if page_no == 1 {
            out.extend_from_slice(&salt);
        }
        out.extend_from_slice(&ciphertext);
        out.extend_from_slice(&iv);
        out.extend_from_slice(&stored_hmac);
    }

    Ok(out)
}

#[cfg(test)]
pub(crate) fn decrypt_masterdb_for_test(raw: &[u8]) -> Vec<u8> {
    decrypt_masterdb(raw).expect("decrypt fixture for test setup")
}

#[cfg(test)]
pub(crate) fn encrypt_masterdb_for_test(plaintext: &[u8]) -> Vec<u8> {
    encrypt_masterdb(plaintext).expect("encrypt fixture for test setup")
}

/// Reads a Rekordbox `master.db` file and returns its path→TrackID index.
///
/// Read-only: no write path exists here (and none is planned — see M8,
/// frozen). The decrypted database is only ever held in memory
/// (`Connection::deserialize_read_exact`, read-only flag set); nothing is
/// written to disk.
///
/// # Errors
///
/// Fails fast on any HMAC mismatch (never returns unverified data), on I/O
/// errors, or if the reassembled buffer is rejected by SQLite.
pub fn read_rekordbox_masterdb(path: &Path) -> Result<RekordboxIndex, MasterDbError> {
    let plaintext = read_and_decrypt_cached(path)?;

    let mut conn =
        Connection::open_in_memory().map_err(|e| MasterDbError::Sqlite(e.to_string()))?;
    let len = plaintext.len();
    conn.deserialize_read_exact(rusqlite::MAIN_DB, Cursor::new(plaintext), len, true)
        .map_err(|e| MasterDbError::Sqlite(e.to_string()))?;

    let mut stmt = conn
        .prepare("SELECT ID, FolderPath FROM djmdContent")
        .map_err(|e| MasterDbError::Sqlite(e.to_string()))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(RekordboxTrack {
                track_id: row.get(0)?,
                folder_path: row.get(1)?,
            })
        })
        .map_err(|e| MasterDbError::Sqlite(e.to_string()))?;

    let mut tracks = Vec::new();
    for row in rows {
        tracks.push(row.map_err(|e| MasterDbError::Sqlite(e.to_string()))?);
    }

    Ok(RekordboxIndex { tracks })
}

/// Reads `djmdPlaylist.Name` for every playlist in `master.db`, keyed by
/// `ID`. Display-only — never consumed by the detect/write engine itself,
/// which only ever needs playlist `ID`s. Added for the M8 Tier 2 UI screen
/// (`docs/superpowers/plans/2026-07-08-m8-tier2-ui-screen.md`): a duplicate
/// group's `playlist_id`/`content_id` alone aren't actionable information
/// for a user, so the UI needs the human-readable name alongside them.
pub fn read_playlist_names(
    path: &Path,
) -> Result<std::collections::HashMap<String, String>, MasterDbError> {
    let plaintext = read_and_decrypt_cached(path)?;

    let mut conn =
        Connection::open_in_memory().map_err(|e| MasterDbError::Sqlite(e.to_string()))?;
    let len = plaintext.len();
    conn.deserialize_read_exact(rusqlite::MAIN_DB, Cursor::new(plaintext), len, true)
        .map_err(|e| MasterDbError::Sqlite(e.to_string()))?;

    let mut stmt = conn
        .prepare("SELECT ID, Name FROM djmdPlaylist")
        .map_err(|e| MasterDbError::Sqlite(e.to_string()))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| MasterDbError::Sqlite(e.to_string()))?;

    let mut names = std::collections::HashMap::new();
    for row in rows {
        let (id, name) = row.map_err(|e| MasterDbError::Sqlite(e.to_string()))?;
        names.insert(id, name);
    }
    Ok(names)
}

/// Whether a Rekordbox process is currently running, checked by partial,
/// case-insensitive process-name match ("rekordbox" matches both
/// `rekordbox.exe` on Windows and `rekordbox` on macOS). Equivalent to
/// `pyrekordbox.utils.get_rekordbox_pid()`, which the M8 write-path design
/// (`docs/superpowers/specs/2026-07-06-m8-masterdb-write-path-rust-design-v2.md`)
/// requires as a guard *before* opening the file — the SQLite "database is
/// locked" exception is only a fallback safety net, never the primary
/// guard (Éval 7, `docs/ressources-externes.md`).
pub(crate) fn is_rekordbox_running() -> bool {
    // `System::new_all()` already loads all process info at construction —
    // no separate refresh call needed for a one-shot check like this.
    let sys = System::new_all();
    sys.processes().values().any(|p| {
        p.name()
            .to_string_lossy()
            .to_lowercase()
            .contains("rekordbox")
    })
}

/// Copies `master.db` + `masterPlaylists6.xml` from `pioneer_dir` into
/// `backup_dir` (created if missing), then verifies the copied `master.db`
/// actually decrypts (full HMAC check on every page) before returning `Ok`
/// — a backup that can't be read back is worse than no backup, so this
/// fails fast rather than trusting a raw file copy blindly.
///
/// On any failure *after* `backup_dir` starts getting populated (XML copy
/// failing after `master.db` copied, or the readability check failing),
/// this removes whatever this call already copied into `backup_dir` before
/// returning the error — a partial or known-unreadable `master.db` left
/// behind under the fixed filename would otherwise look like a valid
/// backup to a later `restore_rekordbox_backup` call. Cleanup uses `.ok()`
/// so a failure to remove never masks the real error being returned.
pub(crate) fn backup_rekordbox_files(
    pioneer_dir: &Path,
    backup_dir: &Path,
) -> Result<(), MasterDbError> {
    std::fs::create_dir_all(backup_dir).map_err(|e| MasterDbError::Io(e.to_string()))?;

    let src_db = pioneer_dir.join("master.db");
    let dst_db = backup_dir.join("master.db");
    std::fs::copy(&src_db, &dst_db).map_err(|e| MasterDbError::Io(e.to_string()))?;

    let src_xml = pioneer_dir.join("masterPlaylists6.xml");
    let dst_xml = backup_dir.join("masterPlaylists6.xml");
    if let Err(e) = std::fs::copy(&src_xml, &dst_xml) {
        std::fs::remove_file(&dst_db).ok();
        return Err(MasterDbError::Io(e.to_string()));
    }

    // Verify the backup is actually readable before trusting it.
    if let Err(e) = read_rekordbox_masterdb(&dst_db) {
        std::fs::remove_file(&dst_db).ok();
        std::fs::remove_file(&dst_xml).ok();
        return Err(e);
    }
    Ok(())
}

/// Restores `master.db` + `masterPlaylists6.xml` from `backup_dir` back into
/// `pioneer_dir`, overwriting the live files. Used both as a user-triggered
/// rollback and internally by the write path when post-write verification
/// fails (see Task 6).
pub(crate) fn restore_rekordbox_backup(
    pioneer_dir: &Path,
    backup_dir: &Path,
) -> Result<(), MasterDbError> {
    std::fs::copy(backup_dir.join("master.db"), pioneer_dir.join("master.db"))
        .map_err(|e| MasterDbError::Io(e.to_string()))?;
    std::fs::copy(
        backup_dir.join("masterPlaylists6.xml"),
        pioneer_dir.join("masterPlaylists6.xml"),
    )
    .map_err(|e| MasterDbError::Io(e.to_string()))?;
    Ok(())
}

/// Shared write safety envelope for every master.db write path (Tier 1/2/3):
/// refuse if Rekordbox is running → backup → decrypt into an in-memory
/// connection → run `mutate` inside a transaction → commit → re-encrypt →
/// atomic rename → run `verify` against the freshly written file → on
/// verification failure, automatically restore the backup and report which
/// of the two rollback outcomes happened.
///
/// `mutate` owns all row changes (including any USN bumps) via the open
/// transaction — this function only owns the crypto/IO/rollback plumbing
/// around it, never the SQL. `verify` re-reads the live file (a fresh
/// connection, not the in-memory one just written) and returns a plain
/// message describing what didn't match if verification fails; that message
/// becomes the `WriteVerificationFailedRolledBack`/`RollbackFailed` payload.
///
/// Extracted 2026-07-09 from `repair_track_path`/`dedup_playlist_group`/
/// `sync_track_metadata`, which each reimplemented this ~40-line sequence
/// verbatim — a safety fix on this path previously had to be applied three
/// times by hand.
fn with_masterdb_write(
    pioneer_dir: &Path,
    backup_dir: &Path,
    mutate: impl FnOnce(&Transaction, &str) -> Result<(), MasterDbError>,
    verify: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<(), MasterDbError> {
    if is_rekordbox_running() {
        return Err(MasterDbError::RekordboxRunning);
    }

    let db_path = pioneer_dir.join("master.db");
    backup_rekordbox_files(pioneer_dir, backup_dir)?;

    let raw = std::fs::read(&db_path).map_err(|e| MasterDbError::Io(e.to_string()))?;
    let plaintext = decrypt_masterdb(&raw)?;

    let mut conn =
        Connection::open_in_memory().map_err(|e| MasterDbError::Sqlite(e.to_string()))?;
    let len = plaintext.len();
    conn.deserialize_read_exact(rusqlite::MAIN_DB, Cursor::new(plaintext), len, false)
        .map_err(|e| MasterDbError::Sqlite(e.to_string()))?;

    let now = chrono::Local::now()
        .format("%Y-%m-%d %H:%M:%S%.6f")
        .to_string();
    let tx = conn
        .transaction()
        .map_err(|e| MasterDbError::Sqlite(e.to_string()))?;

    mutate(&tx, &now)?;

    tx.commit()
        .map_err(|e| MasterDbError::Sqlite(e.to_string()))?;

    let plaintext2 = conn
        .serialize(rusqlite::MAIN_DB)
        .map_err(|e| MasterDbError::Sqlite(e.to_string()))?
        .to_vec();
    let raw2 = encrypt_masterdb(&plaintext2)?;

    let tmp_path = pioneer_dir.join("master.db.sift-write-tmp");
    std::fs::write(&tmp_path, &raw2).map_err(|e| MasterDbError::Io(e.to_string()))?;
    if let Err(e) = std::fs::rename(&tmp_path, &db_path) {
        // The live master.db is untouched (rename never started), but don't
        // leave the temp file behind for a future glob/user to mistake for
        // something real.
        std::fs::remove_file(&tmp_path).ok();
        return Err(MasterDbError::Io(e.to_string()));
    }

    match verify(&db_path) {
        Ok(()) => Ok(()),
        Err(msg) => {
            log::error!("rekordbox master.db write verification failed, restoring backup: {msg}");
            match restore_rekordbox_backup(pioneer_dir, backup_dir) {
                Ok(()) => Err(MasterDbError::WriteVerificationFailedRolledBack(msg)),
                Err(restore_err) => {
                    log::error!(
                        "rekordbox master.db backup restore ALSO failed after a failed write — \
                         master.db may be in an inconsistent state: {restore_err}"
                    );
                    Err(MasterDbError::WriteVerificationFailedRollbackFailed(
                        format!("{msg}; rollback also failed: {restore_err}"),
                    ))
                }
            }
        }
    }
}

/// Repairs one track's `FolderPath`/`FileNameL`/`FileNameS` in a Rekordbox
/// `master.db`, bumping the global USN counter (`agentRegistry`) and the
/// row's own `rb_local_usn`/`updated_at`, per the M8 Tier 1 design
/// (`docs/superpowers/specs/2026-07-06-m8-masterdb-write-path-rust-design-v2.md`).
///
/// Deliberately does **not** touch `masterPlaylists6.xml`. The M8 spike
/// found that a pure `FolderPath` change doesn't semantically require an
/// XML resync (design doc Tier 1 section), but the spike's own real-Rekordbox
/// acceptance test happened to run through `pyrekordbox`, which rewrites the
/// XML as a side effect of *any* `commit()` — so that spike proved
/// "XML-rewritten copies are accepted", not "leaving the XML untouched is
/// equally accepted". This function takes the documented-but-not-fully-proven
/// position that leaving it untouched is fine (no playlists are touched by
/// Tier 1) — flagged here so a future session doesn't mistake this for a
/// fully closed question.
///
/// Also deliberately does **not** touch `Analysed`/`AnalysisUpdated`/
/// `CueUpdated` (the M8 non-negotiable rule — metadata/path writes must
/// never look like an analysis change).
///
/// Safety sequence owned by `with_masterdb_write`; this function supplies
/// only the `djmdContent` mutation and the round-trip check via the
/// existing read-only reader.
pub fn repair_track_path(
    pioneer_dir: &Path,
    backup_dir: &Path,
    repair: &PathRepair,
) -> Result<(), MasterDbError> {
    with_masterdb_write(
        pioneer_dir,
        backup_dir,
        |tx, now| {
            let new_usn = bump_global_usn(tx, now)?;
            let rows_changed = tx
                .execute(
                    "UPDATE djmdContent SET FolderPath = ?1, FileNameL = ?2, FileNameS = ?3, rb_local_usn = ?4, updated_at = ?5 WHERE ID = ?6",
                    rusqlite::params![
                        repair.new_folder_path,
                        repair.new_file_name_l,
                        repair.new_file_name_s,
                        new_usn,
                        now,
                        repair.track_id,
                    ],
                )
                .map_err(|e| MasterDbError::Sqlite(e.to_string()))?;
            if rows_changed != 1 {
                return Err(MasterDbError::TrackNotFound {
                    track_id: repair.track_id.clone(),
                });
            }
            Ok(())
        },
        |db_path| {
            let index = read_rekordbox_masterdb(db_path).map_err(|e| e.to_string())?;
            let ok = index
                .tracks
                .iter()
                .any(|t| t.track_id == repair.track_id && t.folder_path == repair.new_folder_path);
            if ok {
                Ok(())
            } else {
                Err(format!(
                    "track {} not found with expected path after write",
                    repair.track_id
                ))
            }
        },
    )
}

/// Removes every extra occurrence in `group.remove` from `djmdSongPlaylist`,
/// keeping `group.keep` untouched, per the M8 Tier 2 design
/// (`docs/superpowers/specs/2026-07-06-m8-masterdb-write-path-rust-design-v2.md`,
/// Tier 2 section). Bumps the global `agentRegistry` USN counter once per
/// deleted row — a deleted row has no `rb_local_usn` of its own to stamp,
/// unlike `repair_track_path`'s in-place `UPDATE`.
///
/// Deliberately does **not** touch `djmdPlaylist`, `masterPlaylists6.xml`,
/// or `TrackNo` on any surviving row (see this function's module-level scope
/// note).
///
/// Safety sequence owned by `with_masterdb_write`; this function supplies
/// only the per-entry `djmdSongPlaylist` deletion and the round-trip check
/// via `detect_playlist_duplicates`.
#[allow(dead_code)]
pub fn dedup_playlist_group(
    pioneer_dir: &Path,
    backup_dir: &Path,
    group: &PlaylistDuplicateGroup,
) -> Result<(), MasterDbError> {
    if group.remove.is_empty() {
        return Err(MasterDbError::NoDuplicatesToRemove);
    }

    with_masterdb_write(
        pioneer_dir,
        backup_dir,
        |tx, now| {
            for entry in &group.remove {
                bump_global_usn(tx, now)?;
                let rows_changed = tx
                    .execute(
                        "DELETE FROM djmdSongPlaylist WHERE ID = ?1",
                        rusqlite::params![entry.song_playlist_id],
                    )
                    .map_err(|e| MasterDbError::Sqlite(e.to_string()))?;
                if rows_changed != 1 {
                    return Err(MasterDbError::SongPlaylistEntryNotFound {
                        song_playlist_id: entry.song_playlist_id.clone(),
                    });
                }
            }
            Ok(())
        },
        |db_path| {
            let remaining = detect_playlist_duplicates(db_path).map_err(|e| e.to_string())?;
            let still_duplicated = remaining
                .iter()
                .any(|g| g.playlist_id == group.playlist_id && g.content_id == group.content_id);
            if still_duplicated {
                Err(format!(
                    "playlist {} / content {} still has duplicates after dedup",
                    group.playlist_id, group.content_id
                ))
            } else {
                Ok(())
            }
        },
    )
}

/// The only 3 tables `find_or_create_named_row` is allowed to touch — same
/// schema (`ID, Name, UUID, rb_data_status, rb_local_data_status,
/// rb_local_deleted, rb_local_synced, usn, rb_local_usn, created_at,
/// updated_at`), verified identical against a real `master.db` copy
/// (2026-07-09, see design doc Tier 3 section). Whitelisted rather than
/// trusted from the caller: the table name is always a Sift-internal
/// constant, never user input, but this still fails fast instead of
/// building SQL from an unchecked string.
const FK_TABLES: [&str; 3] = ["djmdArtist", "djmdGenre", "djmdLabel"];

/// Bumps the global `agentRegistry.localUpdateCount` USN by one and returns
/// the new value — identical query used by `repair_track_path` and
/// `dedup_playlist_group`, factored out here since Tier 3 may need to call
/// it up to 4 times in a single sync (one per newly-created FK row, plus
/// one for the `djmdContent` row itself).
fn bump_global_usn(tx: &Transaction, now: &str) -> Result<i64, MasterDbError> {
    let old_usn: i64 = tx
        .query_row(
            "SELECT int_1 FROM agentRegistry WHERE registry_id = 'localUpdateCount'",
            [],
            |row| row.get(0),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => MasterDbError::RegistryRowMissing,
            other => MasterDbError::Sqlite(other.to_string()),
        })?;
    let new_usn = old_usn + 1;
    tx.execute(
        "UPDATE agentRegistry SET int_1 = ?1, updated_at = ?2 WHERE registry_id = 'localUpdateCount'",
        rusqlite::params![new_usn, now],
    )
    .map_err(|e| MasterDbError::Sqlite(e.to_string()))?;
    Ok(new_usn)
}

/// Rekordbox `djmdArtist`/`djmdGenre`/`djmdLabel` IDs observed in the wild
/// (`274555000`, `1521864440`, `3689289451`) are unsigned 32-bit integers
/// with no documented generation scheme — random draw + uniqueness check
/// mirrors that absence of visible pattern, rather than inventing a
/// sequence Rekordbox itself doesn't use.
fn generate_free_id(tx: &Transaction, table: &str) -> Result<String, MasterDbError> {
    const MAX_ATTEMPTS: u32 = 1000;
    for _ in 0..MAX_ATTEMPTS {
        let candidate: u32 = OsRng.gen();
        let exists: bool = tx
            .query_row(
                &format!("SELECT 1 FROM {table} WHERE ID = ?1"),
                rusqlite::params![candidate.to_string()],
                |_| Ok(true),
            )
            .optional()
            .map_err(|e| MasterDbError::Sqlite(e.to_string()))?
            .unwrap_or(false);
        if !exists {
            return Ok(candidate.to_string());
        }
    }
    Err(MasterDbError::IdGenerationExhausted {
        table: table.to_string(),
    })
}

/// Finds a `djmdArtist`/`djmdGenre`/`djmdLabel` row by exact `Name` match
/// and returns its `ID`; if none exists, creates one and returns the new
/// `ID`. Empirically mirrors what Rekordbox's own "Reload Tag" does
/// (M8 spikes 6/7): reuse the existing row untouched when the name already
/// exists (spike 7), or create a new row with a fresh `ID`+`UUID` and a
/// bumped `rb_local_usn` when it doesn't (spike 6) — never update an
/// existing row's `Name` in place, never delete anything.
///
/// Matching is trim+case-insensitive (`COLLATE NOCASE`, ASCII-only — SQLite's
/// built-in collation does not fold accented characters, e.g. "é"≠"É"; a
/// known, accepted limit, not a silent gap) so that "Eat Static" and
/// " eat static " resolve to the same row instead of spawning a cosmetic
/// duplicate (residual risk #1 from the design doc, closed 2026-07-09 —
/// untested whether Rekordbox's own matching goes any further than this,
/// e.g. accent folding, but this closes the common case cheaply). A newly
/// created row stores the trimmed-but-original-case name, never
/// force-lowercased — only the *comparison* is normalized, not the stored
/// value.
fn find_or_create_named_row(
    tx: &Transaction,
    table: &str,
    name: &str,
    now: &str,
) -> Result<String, MasterDbError> {
    if !FK_TABLES.contains(&table) {
        return Err(MasterDbError::UnknownFkTable {
            table: table.to_string(),
        });
    }
    let trimmed = name.trim();

    let existing: Option<String> = tx
        .query_row(
            &format!("SELECT ID FROM {table} WHERE TRIM(Name) = TRIM(?1) COLLATE NOCASE"),
            rusqlite::params![trimmed],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| MasterDbError::Sqlite(e.to_string()))?;
    if let Some(id) = existing {
        return Ok(id);
    }

    let new_id = generate_free_id(tx, table)?;
    let new_uuid = Uuid::new_v4().to_string();
    bump_global_usn(tx, now)?;
    tx.execute(
        &format!(
            "INSERT INTO {table} (ID, Name, UUID, rb_data_status, rb_local_data_status, \
             rb_local_deleted, rb_local_synced, usn, rb_local_usn, created_at, updated_at) \
             VALUES (?1, ?2, ?3, 0, 0, 0, 0, NULL, ?4, ?5, ?5)"
        ),
        rusqlite::params![new_id, trimmed, new_uuid, 1_i64, now],
    )
    .map_err(|e| MasterDbError::Sqlite(e.to_string()))?;
    Ok(new_id)
}

/// Splits an `ImagePath` filename into its (stem, extension) and derives
/// the sibling "_m"/"_s" variant filenames Rekordbox maintains alongside
/// the full-size file — same directory, same extension, `_m`/`_s` suffix
/// inserted before the extension (observed on real Rekordbox data, spike 8:
/// `artwork.jpg` / `artwork_m.jpg` / `artwork_s.jpg`).
/// Joins `image_path` onto `pioneer_dir/share` component-by-component,
/// refusing `..` segments and drive-prefixed segments (e.g. `C:`) instead of
/// handing the raw string to a single `PathBuf::join` — the same containment
/// discipline as `library::safe_join`, needed because `ImagePath` comes from
/// `djmdContent` in a linked master.db, which a user can point at an
/// attacker-supplied file via Sift's own "link Rekordbox XML" picker.
#[allow(dead_code)]
fn resolve_artwork_variants(
    pioneer_dir: &Path,
    image_path: &str,
) -> Result<(PathBuf, PathBuf, PathBuf), MasterDbError> {
    let share_root = pioneer_dir.join("share");
    let mut full = share_root.clone();
    for raw in image_path.split(['/', '\\']) {
        if raw.is_empty() || raw == "." {
            continue;
        }
        if raw == ".." || raw.contains(':') {
            return Err(MasterDbError::ArtworkPathEscapesRoot {
                path: image_path.to_string(),
            });
        }
        full.push(raw);
    }
    if full == share_root {
        return Err(MasterDbError::ArtworkPathEscapesRoot {
            path: image_path.to_string(),
        });
    }
    let stem = full
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let ext = full
        .extension()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "jpg".to_string());
    let parent = full.parent().map(Path::to_path_buf).unwrap_or_default();
    let medium = parent.join(format!("{stem}_m.{ext}"));
    let small = parent.join(format!("{stem}_s.{ext}"));
    Ok((full, medium, small))
}

/// M8 Tier 3 — pochette. Overwrites the 3 cached artwork files Rekordbox
/// keeps for a track (`pioneer_dir/share/<ImagePath>` and its `_m`/`_s`
/// siblings) in place, resizing `cover_bytes` to match each existing
/// variant's exact dimensions. Never touches `master.db` — `ImagePath`
/// itself never changes (confirmed by spike 8), this only replaces file
/// bytes on disk.
///
/// Refuses (no silent fallback) when: Rekordbox is running, the track has
/// no `ImagePath` (`NoArtworkPath`), or any of the 3 variant files is
/// missing on disk (`ArtworkVariantMissing`) — the "no existing artwork at
/// all" case was never observed at spike 8, so this never guesses a
/// find-or-create-style behavior for it.
#[allow(dead_code)]
pub fn sync_track_artwork(
    pioneer_dir: &Path,
    backup_dir: &Path,
    track_id: &str,
    cover_bytes: &[u8],
) -> Result<(), MasterDbError> {
    if is_rekordbox_running() {
        return Err(MasterDbError::RekordboxRunning);
    }

    let db_path = pioneer_dir.join("master.db");
    let raw = std::fs::read(&db_path).map_err(|e| MasterDbError::Io(e.to_string()))?;
    let plaintext = decrypt_masterdb(&raw)?;
    let mut conn =
        Connection::open_in_memory().map_err(|e| MasterDbError::Sqlite(e.to_string()))?;
    let len = plaintext.len();
    conn.deserialize_read_exact(rusqlite::MAIN_DB, Cursor::new(plaintext), len, false)
        .map_err(|e| MasterDbError::Sqlite(e.to_string()))?;

    let image_path: Option<String> = conn
        .query_row(
            "SELECT ImagePath FROM djmdContent WHERE ID = ?1",
            rusqlite::params![track_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| MasterDbError::Sqlite(e.to_string()))?
        .ok_or_else(|| MasterDbError::TrackNotFound {
            track_id: track_id.to_string(),
        })?;
    let image_path = image_path.filter(|p| !p.trim().is_empty()).ok_or_else(|| {
        MasterDbError::NoArtworkPath {
            track_id: track_id.to_string(),
        }
    })?;

    let (full, medium, small) = resolve_artwork_variants(pioneer_dir, &image_path)?;
    for target in [&full, &medium, &small] {
        if !target.exists() {
            return Err(MasterDbError::ArtworkVariantMissing {
                path: target.to_string_lossy().to_string(),
            });
        }
    }

    // Backup the 3 live files before touching any of them (own backup,
    // distinct from backup_rekordbox_files — these aren't master.db/xml).
    std::fs::create_dir_all(backup_dir).map_err(|e| MasterDbError::Io(e.to_string()))?;
    let mut backups: Vec<(PathBuf, PathBuf)> = Vec::new();
    for target in [&full, &medium, &small] {
        let file_name = target
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let backup_path = backup_dir.join(format!("artwork-{file_name}"));
        std::fs::copy(target, &backup_path).map_err(|e| MasterDbError::Io(e.to_string()))?;
        backups.push(((*target).clone(), backup_path));
    }

    let restore_all = |backups: &[(PathBuf, PathBuf)]| -> Result<(), MasterDbError> {
        for (target, backup_path) in backups {
            std::fs::copy(backup_path, target).map_err(|e| MasterDbError::Io(e.to_string()))?;
        }
        Ok(())
    };

    let cover = match image::load_from_memory(cover_bytes) {
        Ok(img) => img,
        Err(e) => return Err(MasterDbError::Io(format!("cover decode failed: {e}"))),
    };

    let write_result = (|| -> Result<(), MasterDbError> {
        for target in [&full, &medium, &small] {
            let (w, h) = image::image_dimensions(target)
                .map_err(|e| MasterDbError::Io(format!("reading dimensions of {target:?}: {e}")))?;
            let resized = cover.resize_exact(w, h, image::imageops::FilterType::Lanczos3);
            let mut buf: Vec<u8> = Vec::new();
            {
                let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 90);
                encoder
                    .write_image(
                        resized.to_rgb8().as_raw(),
                        w,
                        h,
                        image::ExtendedColorType::Rgb8,
                    )
                    .map_err(|e| MasterDbError::Io(format!("jpeg encode failed: {e}")))?;
            }
            let tmp = target.with_extension("sift-write-tmp");
            std::fs::write(&tmp, &buf).map_err(|e| MasterDbError::Io(e.to_string()))?;
            if let Err(e) = std::fs::rename(&tmp, target) {
                std::fs::remove_file(&tmp).ok();
                return Err(MasterDbError::Io(e.to_string()));
            }
        }
        Ok(())
    })();

    if let Err(write_err) = write_result {
        log::error!("rekordbox artwork write failed mid-loop, restoring backup: {write_err}");
        return Err(match restore_all(&backups) {
            Ok(()) => write_err,
            Err(restore_err) => {
                log::error!(
                    "rekordbox artwork backup restore ALSO failed after a failed write — \
                     artwork files may be in an inconsistent state: {restore_err}"
                );
                MasterDbError::ArtworkWriteFailedRollbackFailed(format!(
                    "{write_err}; rollback also failed: {restore_err}"
                ))
            }
        });
    }

    // Round-trip verify: reread each written file and confirm it decodes
    // and still matches the dimensions it had before (the ones we resized
    // to) — not just that the write syscall succeeded.
    let verify = || -> Result<(), MasterDbError> {
        for (target, backup_path) in &backups {
            let expected = image::image_dimensions(backup_path)
                .map_err(|e| MasterDbError::Io(format!("reading backup dimensions: {e}")))?;
            let got = image::image_dimensions(target)
                .map_err(|e| MasterDbError::Io(format!("reading written dimensions: {e}")))?;
            if got != expected {
                return Err(MasterDbError::Io(format!(
                    "dimension mismatch after write for {target:?}: expected {expected:?}, got {got:?}"
                )));
            }
        }
        Ok(())
    };

    match verify() {
        Ok(()) => Ok(()),
        Err(verify_err) => {
            log::error!(
                "rekordbox artwork write verification failed, restoring backup: {verify_err}"
            );
            match restore_all(&backups) {
                Ok(()) => Err(MasterDbError::ArtworkWriteVerificationFailedRolledBack(
                    verify_err.to_string(),
                )),
                Err(restore_err) => {
                    log::error!(
                        "rekordbox artwork backup restore ALSO failed after a failed verification — \
                         artwork files may be in an inconsistent state: {restore_err}"
                    );
                    Err(MasterDbError::ArtworkWriteVerificationFailedRollbackFailed(
                        format!("{verify_err}; rollback also failed: {restore_err}"),
                    ))
                }
            }
        }
    }
}

/// M8 Tier 3 — writes Sift's own tagging output directly into `master.db`,
/// so Rekordbox reflects it without the user having to manually
/// right-click → "Reload Tag" per track. Scope is exactly the fields
/// `tagging::write_tags_full` writes (see `MetadataSync` doc) — cover and
/// album are out of scope, Sift never writes those tags.
///
/// Safety sequence identical to `repair_track_path`/`dedup_playlist_group`:
/// refuse if Rekordbox is running → backup → decrypt → mutate inside a
/// transaction → re-encrypt → atomic write → round-trip verify (fresh
/// connection) → automatic rollback on verification failure.
///
/// Never touches `Analysed`/`AnalysisUpdated`/`CueUpdated` — the M8
/// non-negotiable invariant, unchanged since Tier 1.
#[allow(dead_code)]
pub fn sync_track_metadata(
    pioneer_dir: &Path,
    backup_dir: &Path,
    sync: &MetadataSync,
) -> Result<(), MasterDbError> {
    with_masterdb_write(
        pioneer_dir,
        backup_dir,
        |tx, now| {
            // Confirm the track exists before touching any FK table — a
            // find_or_create for a non-existent track would still create
            // orphaned rows even though the final djmdContent UPDATE fails.
            let track_exists: bool = tx
                .query_row(
                    "SELECT 1 FROM djmdContent WHERE ID = ?1",
                    rusqlite::params![sync.track_id],
                    |_| Ok(true),
                )
                .optional()
                .map_err(|e| MasterDbError::Sqlite(e.to_string()))?
                .unwrap_or(false);
            if !track_exists {
                return Err(MasterDbError::TrackNotFound {
                    track_id: sync.track_id.clone(),
                });
            }

            let mut any_field_set = false;

            if let Some(artist) = &sync.artist {
                let artist_id = find_or_create_named_row(tx, "djmdArtist", artist, now)?;
                tx.execute(
                    "UPDATE djmdContent SET ArtistID = ?1 WHERE ID = ?2",
                    rusqlite::params![artist_id, sync.track_id],
                )
                .map_err(|e| MasterDbError::Sqlite(e.to_string()))?;
                any_field_set = true;
            }
            if let Some(genre) = &sync.genre {
                let genre_id = find_or_create_named_row(tx, "djmdGenre", genre, now)?;
                tx.execute(
                    "UPDATE djmdContent SET GenreID = ?1 WHERE ID = ?2",
                    rusqlite::params![genre_id, sync.track_id],
                )
                .map_err(|e| MasterDbError::Sqlite(e.to_string()))?;
                any_field_set = true;
            }
            if let Some(label) = &sync.label {
                let label_id = find_or_create_named_row(tx, "djmdLabel", label, now)?;
                tx.execute(
                    "UPDATE djmdContent SET LabelID = ?1 WHERE ID = ?2",
                    rusqlite::params![label_id, sync.track_id],
                )
                .map_err(|e| MasterDbError::Sqlite(e.to_string()))?;
                any_field_set = true;
            }
            if let Some(title) = &sync.title {
                tx.execute(
                    "UPDATE djmdContent SET Title = ?1 WHERE ID = ?2",
                    rusqlite::params![title, sync.track_id],
                )
                .map_err(|e| MasterDbError::Sqlite(e.to_string()))?;
                any_field_set = true;
            }
            if let Some(year) = sync.year {
                tx.execute(
                    "UPDATE djmdContent SET ReleaseYear = ?1 WHERE ID = ?2",
                    rusqlite::params![year, sync.track_id],
                )
                .map_err(|e| MasterDbError::Sqlite(e.to_string()))?;
                any_field_set = true;
            }

            if any_field_set {
                let content_usn = bump_global_usn(tx, now)?;
                tx.execute(
                    "UPDATE djmdContent SET rb_local_usn = ?1, updated_at = ?2, \
                     TrackInfoUpdated = CAST(CAST(TrackInfoUpdated AS INTEGER) + 1 AS TEXT) \
                     WHERE ID = ?3",
                    rusqlite::params![content_usn, now, sync.track_id],
                )
                .map_err(|e| MasterDbError::Sqlite(e.to_string()))?;
            }

            Ok(())
        },
        |db_path| {
            // Round-trip verify on a fresh connection: reopen, decrypt, and
            // confirm the fields we set are actually visible — not just that
            // the write syscall succeeded.
            let raw3 = std::fs::read(db_path).map_err(|e| e.to_string())?;
            let plaintext3 = decrypt_masterdb(&raw3).map_err(|e| e.to_string())?;
            let mut conn3 = Connection::open_in_memory().map_err(|e| e.to_string())?;
            let len3 = plaintext3.len();
            conn3
                .deserialize_read_exact(rusqlite::MAIN_DB, Cursor::new(plaintext3), len3, false)
                .map_err(|e| e.to_string())?;

            if let Some(title) = &sync.title {
                let got: String = conn3
                    .query_row(
                        "SELECT Title FROM djmdContent WHERE ID = ?1",
                        rusqlite::params![sync.track_id],
                        |row| row.get(0),
                    )
                    .map_err(|e| e.to_string())?;
                if &got != title {
                    return Err(format!(
                        "Title mismatch after write: expected {title:?}, got {got:?}"
                    ));
                }
            }
            if let Some(artist) = &sync.artist {
                let got: String = conn3
                    .query_row(
                        "SELECT a.Name FROM djmdContent c JOIN djmdArtist a ON a.ID = c.ArtistID \
                         WHERE c.ID = ?1",
                        rusqlite::params![sync.track_id],
                        |row| row.get(0),
                    )
                    .map_err(|e| e.to_string())?;
                // Compare with the same trim+case-insensitive rule
                // `find_or_create_named_row` matched on — a reused existing
                // row legitimately keeps its own stored casing/whitespace,
                // which can differ from the raw incoming tag value.
                if !got.trim().eq_ignore_ascii_case(artist.trim()) {
                    return Err(format!(
                        "Artist mismatch after write: expected {artist:?}, got {got:?}"
                    ));
                }
            }
            Ok(())
        },
    )
}

// Fixture provenance: `tests/fixtures/rekordbox_master.db` is a synthetic
// SQLCipher v4 database (3 fake tracks, 1 fake playlist, 1 fake
// agentRegistry row, no personal data), generated by
// `scripts/make-rekordbox-fixture.py` — regenerate with
// `python scripts/make-rekordbox-fixture.py` only if the fixture's
// schema/data needs to change; never copy data from a real Rekordbox
// library into this file.
#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/rekordbox_master.db"
    );

    #[test]
    fn deobfuscate_key_matches_pyrekordbox_reference() {
        let key = deobfuscate_key().expect("deobfuscate");
        assert_eq!(
            key,
            "402fd482c38817c35ffa8ffb8c7d93143b749e7d315df7a81732a1ff43608497"
        );
    }

    #[test]
    fn reads_fixture_tracks() {
        let index = read_rekordbox_masterdb(Path::new(FIXTURE)).expect("read fixture");
        assert_eq!(index.tracks.len(), 3);
        let mut ids: Vec<&str> = index.tracks.iter().map(|t| t.track_id.as_str()).collect();
        ids.sort_unstable();
        assert_eq!(ids, vec!["40000001", "40000002", "40000003"]);

        let track1 = index
            .tracks
            .iter()
            .find(|t| t.track_id == "40000001")
            .expect("track 40000001 present");
        assert_eq!(track1.folder_path, "D:/FIXTURE/track1.mp3");
    }

    #[test]
    fn rejects_truncated_file() {
        let err = decrypt_masterdb(&[0u8; 10]).unwrap_err();
        assert_eq!(err, MasterDbError::FileTooShort);
    }

    #[test]
    fn rejects_file_size_not_a_multiple_of_page_size() {
        let mut raw = std::fs::read(FIXTURE).expect("read fixture bytes");
        // Fixture is an exact multiple of PAGE_SIZE (7 full pages); truncate
        // a few trailing bytes to simulate a partial last page (e.g.
        // interrupted copy / partial read over a network share).
        let truncated_len = raw.len() - 10;
        raw.truncate(truncated_len);
        let err = decrypt_masterdb(&raw).unwrap_err();
        assert_eq!(err, MasterDbError::TruncatedFile { len: truncated_len });
    }

    #[test]
    fn rejects_corrupted_page_hmac() {
        let mut raw = std::fs::read(FIXTURE).expect("read fixture bytes");
        // Flip a byte inside page 1's ciphertext region (well past the
        // never-encrypted salt) to break its HMAC.
        raw[100] ^= 0xFF;
        let err = decrypt_masterdb(&raw).unwrap_err();
        assert_eq!(err, MasterDbError::HmacMismatch { page: 1 });
    }

    #[test]
    fn fixture_has_tier1_write_columns() {
        let raw = std::fs::read(FIXTURE).expect("read fixture bytes");
        let plaintext = decrypt_masterdb(&raw).expect("decrypt fixture");
        let mut conn = Connection::open_in_memory().expect("open in-memory");
        let len = plaintext.len();
        conn.deserialize_read_exact(rusqlite::MAIN_DB, Cursor::new(plaintext), len, true)
            .expect("deserialize fixture");

        let (file_name_l, rb_local_usn): (String, i64) = conn
            .query_row(
                "SELECT FileNameL, rb_local_usn FROM djmdContent WHERE ID = '40000001'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("djmdContent has FileNameL/rb_local_usn");
        assert_eq!(file_name_l, "track1.mp3");
        assert_eq!(rb_local_usn, 1000);

        let usn: i64 = conn
            .query_row(
                "SELECT int_1 FROM agentRegistry WHERE registry_id = 'localUpdateCount'",
                [],
                |row| row.get(0),
            )
            .expect("agentRegistry has localUpdateCount row");
        assert_eq!(usn, 1000);
    }

    #[test]
    fn fixture_has_a_playlist_duplicate() {
        let raw = std::fs::read(FIXTURE).expect("read fixture bytes");
        let plaintext = decrypt_masterdb(&raw).expect("decrypt fixture");
        let mut conn = Connection::open_in_memory().expect("open in-memory");
        let len = plaintext.len();
        conn.deserialize_read_exact(rusqlite::MAIN_DB, Cursor::new(plaintext), len, true)
            .expect("deserialize fixture");

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM djmdSongPlaylist WHERE PlaylistID = '50000001' AND ContentID = '40000001'",
                [],
                |row| row.get(0),
            )
            .expect("query djmdSongPlaylist");
        assert_eq!(
            count, 2,
            "fixture must have track 40000001 twice in playlist 50000001"
        );
    }

    #[test]
    fn reconstructed_buffer_declares_true_reserve() {
        let raw = std::fs::read(FIXTURE).expect("read fixture bytes");
        let plaintext = decrypt_masterdb(&raw).expect("decrypt fixture");
        // SQLite file header offset 20 = "reserved space per page".
        assert_eq!(plaintext[20], RESERVE as u8);
    }

    #[test]
    fn encrypt_then_decrypt_round_trips_byte_identical() {
        let raw = std::fs::read(FIXTURE).expect("read fixture bytes");
        let plaintext = decrypt_masterdb(&raw).expect("decrypt fixture");

        let reencrypted = encrypt_masterdb(&plaintext).expect("encrypt plaintext");
        assert_eq!(reencrypted.len(), raw.len());

        let roundtripped = decrypt_masterdb(&reencrypted).expect("decrypt reencrypted");
        assert_eq!(roundtripped, plaintext);
    }

    #[test]
    fn encrypt_then_decrypt_still_reads_via_rusqlite() {
        let raw = std::fs::read(FIXTURE).expect("read fixture bytes");
        let plaintext = decrypt_masterdb(&raw).expect("decrypt fixture");
        let reencrypted = encrypt_masterdb(&plaintext).expect("encrypt plaintext");

        std::fs::write(FIXTURE.replace(".db", "_reencrypted_tmp.db"), &reencrypted)
            .expect("write temp reencrypted file");
        let index = read_rekordbox_masterdb(std::path::Path::new(
            &FIXTURE.replace(".db", "_reencrypted_tmp.db"),
        ))
        .expect("read reencrypted file");
        std::fs::remove_file(FIXTURE.replace(".db", "_reencrypted_tmp.db")).ok();

        assert_eq!(index.tracks.len(), 3);
    }

    #[test]
    fn backup_then_restore_round_trips() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let pioneer_dir = tmp.path().join("pioneer");
        let backup_dir = tmp.path().join("backup");
        std::fs::create_dir_all(&pioneer_dir).expect("mkdir pioneer");

        std::fs::copy(FIXTURE, pioneer_dir.join("master.db")).expect("copy fixture as master.db");
        std::fs::write(pioneer_dir.join("masterPlaylists6.xml"), b"<DJ_PLAYLISTS/>")
            .expect("write fake xml");

        backup_rekordbox_files(&pioneer_dir, &backup_dir).expect("backup");
        assert!(backup_dir.join("master.db").exists());
        assert!(backup_dir.join("masterPlaylists6.xml").exists());

        // Corrupt the "live" master.db to simulate a bad write.
        std::fs::write(pioneer_dir.join("master.db"), b"corrupted").expect("corrupt live db");

        restore_rekordbox_backup(&pioneer_dir, &backup_dir).expect("restore");
        let restored = std::fs::read(pioneer_dir.join("master.db")).expect("read restored");
        let original = std::fs::read(FIXTURE).expect("read fixture");
        assert_eq!(restored, original);
    }

    #[test]
    fn backup_rejects_unreadable_masterdb() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let pioneer_dir = tmp.path().join("pioneer");
        let backup_dir = tmp.path().join("backup");
        std::fs::create_dir_all(&pioneer_dir).expect("mkdir pioneer");
        std::fs::write(pioneer_dir.join("master.db"), b"not a real database")
            .expect("write garbage master.db");
        std::fs::write(pioneer_dir.join("masterPlaylists6.xml"), b"<DJ_PLAYLISTS/>")
            .expect("write fake xml");

        let err = backup_rekordbox_files(&pioneer_dir, &backup_dir).unwrap_err();
        assert!(matches!(err, MasterDbError::FileTooShort));
    }

    #[test]
    fn backup_cleans_up_after_verification_failure() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let pioneer_dir = tmp.path().join("pioneer");
        let backup_dir = tmp.path().join("backup");
        std::fs::create_dir_all(&pioneer_dir).expect("mkdir pioneer");
        // Both source files exist, so both copies into backup_dir succeed —
        // the failure happens only at the read_rekordbox_masterdb
        // verification step, after backup_dir is already populated.
        std::fs::write(pioneer_dir.join("master.db"), b"not a real database")
            .expect("write garbage master.db");
        std::fs::write(pioneer_dir.join("masterPlaylists6.xml"), b"<DJ_PLAYLISTS/>")
            .expect("write fake xml");

        let err = backup_rekordbox_files(&pioneer_dir, &backup_dir).unwrap_err();
        assert!(matches!(err, MasterDbError::FileTooShort));

        // The broken master.db copy must not be left behind under the fixed
        // filename — a later restore_rekordbox_backup call against this
        // backup_dir must not find anything to (wrongly) trust.
        assert!(!backup_dir.join("master.db").exists());
        assert!(!backup_dir.join("masterPlaylists6.xml").exists());
    }

    #[test]
    fn backup_cleans_up_after_xml_copy_failure() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let pioneer_dir = tmp.path().join("pioneer");
        let backup_dir = tmp.path().join("backup");
        std::fs::create_dir_all(&pioneer_dir).expect("mkdir pioneer");

        // master.db exists and is valid, but masterPlaylists6.xml is
        // deliberately missing — the master.db copy succeeds first, then
        // the XML copy fails.
        std::fs::copy(FIXTURE, pioneer_dir.join("master.db")).expect("copy fixture as master.db");

        let err = backup_rekordbox_files(&pioneer_dir, &backup_dir).unwrap_err();
        assert!(matches!(err, MasterDbError::Io(_)));

        // The master.db copy that succeeded before the XML failure must not
        // be left behind under the fixed filename.
        assert!(!backup_dir.join("master.db").exists());
    }

    #[test]
    fn detect_playlist_duplicates_finds_the_fixture_duplicate() {
        let groups = detect_playlist_duplicates(Path::new(FIXTURE)).expect("detect");
        assert_eq!(groups.len(), 1, "fixture has exactly one duplicate group");
        let g = &groups[0];
        assert_eq!(g.playlist_id, "50000001");
        assert_eq!(g.content_id, "40000001");
        assert_eq!(g.keep.song_playlist_id, "60000001");
        assert_eq!(g.keep.track_no, 1);
        assert_eq!(g.remove.len(), 1);
        assert_eq!(g.remove[0].song_playlist_id, "60000003");
        assert_eq!(g.remove[0].track_no, 3);
    }

    #[test]
    fn detect_playlist_duplicates_ignores_non_duplicated_entries() {
        let groups = detect_playlist_duplicates(Path::new(FIXTURE)).expect("detect");
        // Track 40000002 appears exactly once (TrackNo 2, playlist 50000001)
        // — must not show up as a group.
        assert!(!groups.iter().any(|g| g.content_id == "40000002"));
    }

    #[test]
    fn read_playlist_names_returns_the_fixture_playlist() {
        let names = read_playlist_names(Path::new(FIXTURE)).expect("read playlist names");
        assert_eq!(names.get("50000001"), Some(&"Fixture Playlist".to_string()));
        assert_eq!(names.len(), 1, "fixture has exactly one playlist");
    }

    #[test]
    fn is_rekordbox_running_does_not_panic() {
        // Can't assert a specific value cross-platform without spawning a
        // real process named "rekordbox" — this is a smoke test proving the
        // sysinfo integration itself doesn't panic. In this test
        // environment Rekordbox is not running, so the honest expectation
        // is `false`; if this ever flakes true on a dev machine that
        // genuinely has Rekordbox open, that is the function working
        // correctly, not a bug.
        let running = is_rekordbox_running();
        assert!(
            !running,
            "Rekordbox should not be running in the test environment"
        );
    }

    #[test]
    fn repair_track_path_updates_path_and_bumps_usn() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let pioneer_dir = tmp.path().join("pioneer");
        let backup_dir = tmp.path().join("backup");
        std::fs::create_dir_all(&pioneer_dir).expect("mkdir pioneer");
        std::fs::copy(FIXTURE, pioneer_dir.join("master.db")).expect("copy fixture as master.db");
        std::fs::write(pioneer_dir.join("masterPlaylists6.xml"), b"<DJ_PLAYLISTS/>")
            .expect("write fake xml");

        let repair = PathRepair {
            track_id: "40000001".to_string(),
            new_folder_path: "D:/FIXTURE/renamed/track1.flac".to_string(),
            new_file_name_l: "track1.flac".to_string(),
            new_file_name_s: "track1.flac".to_string(),
        };
        repair_track_path(&pioneer_dir, &backup_dir, &repair).expect("repair path");

        // Round-trip via the existing read-only reader.
        let index = read_rekordbox_masterdb(&pioneer_dir.join("master.db")).expect("reread");
        let repaired = index
            .tracks
            .iter()
            .find(|t| t.track_id == "40000001")
            .expect("track 40000001 present");
        assert_eq!(repaired.folder_path, "D:/FIXTURE/renamed/track1.flac");

        // Other two tracks untouched.
        let other = index
            .tracks
            .iter()
            .find(|t| t.track_id == "40000002")
            .expect("track 40000002 present");
        assert_eq!(other.folder_path, "D:/FIXTURE/track2.flac");
        assert_eq!(index.tracks.len(), 3);

        // Backup exists and matches the original fixture.
        let backed_up = std::fs::read(backup_dir.join("master.db")).expect("read backup");
        let original = std::fs::read(FIXTURE).expect("read fixture");
        assert_eq!(backed_up, original);
    }

    #[test]
    fn repair_track_path_rejects_unknown_track_id() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let pioneer_dir = tmp.path().join("pioneer");
        let backup_dir = tmp.path().join("backup");
        std::fs::create_dir_all(&pioneer_dir).expect("mkdir pioneer");
        std::fs::copy(FIXTURE, pioneer_dir.join("master.db")).expect("copy fixture as master.db");
        std::fs::write(pioneer_dir.join("masterPlaylists6.xml"), b"<DJ_PLAYLISTS/>")
            .expect("write fake xml");

        let repair = PathRepair {
            track_id: "99999999".to_string(),
            new_folder_path: "D:/nope.mp3".to_string(),
            new_file_name_l: "nope.mp3".to_string(),
            new_file_name_s: "nope.mp3".to_string(),
        };
        let err = repair_track_path(&pioneer_dir, &backup_dir, &repair).unwrap_err();
        assert_eq!(
            err,
            MasterDbError::TrackNotFound {
                track_id: "99999999".to_string()
            }
        );
    }

    #[test]
    fn repair_track_path_handles_long_path_forcing_page_growth() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let pioneer_dir = tmp.path().join("pioneer");
        let backup_dir = tmp.path().join("backup");
        std::fs::create_dir_all(&pioneer_dir).expect("mkdir pioneer");
        std::fs::copy(FIXTURE, pioneer_dir.join("master.db")).expect("copy fixture as master.db");
        std::fs::write(pioneer_dir.join("masterPlaylists6.xml"), b"<DJ_PLAYLISTS/>")
            .expect("write fake xml");

        let original_size = std::fs::metadata(pioneer_dir.join("master.db"))
            .expect("stat before")
            .len();

        // Well past SQLite's per-cell local-payload threshold (~page_size - 35
        // bytes) — this forces the updated row onto an overflow page, growing
        // the file past its original page count. Exercises encrypt_masterdb
        // on a plaintext buffer LARGER than what decrypt_masterdb produced
        // for this file, not just the fixture's original fixed geometry.
        let long_component = "a".repeat(8000);
        let long_path = format!("D:/FIXTURE/{long_component}/track1.mp3");
        let repair = PathRepair {
            track_id: "40000001".to_string(),
            new_folder_path: long_path.clone(),
            new_file_name_l: "track1.mp3".to_string(),
            new_file_name_s: "track1.mp3".to_string(),
        };
        repair_track_path(&pioneer_dir, &backup_dir, &repair).expect("repair path with long path");

        let new_size = std::fs::metadata(pioneer_dir.join("master.db"))
            .expect("stat after")
            .len();
        assert!(
            new_size > original_size,
            "expected the database to grow past its original {original_size} bytes to hold the long path (overflow page), got {new_size}"
        );

        let index = read_rekordbox_masterdb(&pioneer_dir.join("master.db")).expect("reread");
        let repaired = index
            .tracks
            .iter()
            .find(|t| t.track_id == "40000001")
            .expect("track 40000001 present");
        assert_eq!(repaired.folder_path, long_path);
        assert_eq!(index.tracks.len(), 3);
    }

    #[test]
    fn repair_track_path_handles_non_ascii_path() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let pioneer_dir = tmp.path().join("pioneer");
        let backup_dir = tmp.path().join("backup");
        std::fs::create_dir_all(&pioneer_dir).expect("mkdir pioneer");
        std::fs::copy(FIXTURE, pioneer_dir.join("master.db")).expect("copy fixture as master.db");
        std::fs::write(pioneer_dir.join("masterPlaylists6.xml"), b"<DJ_PLAYLISTS/>")
            .expect("write fake xml");

        let accented_path = "D:/Musique/Various Artistes/Beyoncé - Déjà vu (édit).mp3".to_string();
        let repair = PathRepair {
            track_id: "40000002".to_string(),
            new_folder_path: accented_path.clone(),
            new_file_name_l: "Beyoncé - Déjà vu (édit).mp3".to_string(),
            new_file_name_s: "Beyonce.mp3".to_string(),
        };
        repair_track_path(&pioneer_dir, &backup_dir, &repair)
            .expect("repair path with accented characters");

        let index = read_rekordbox_masterdb(&pioneer_dir.join("master.db")).expect("reread");
        let repaired = index
            .tracks
            .iter()
            .find(|t| t.track_id == "40000002")
            .expect("track 40000002 present");
        assert_eq!(repaired.folder_path, accented_path);
    }

    /// M8 Tier 1 real-data gate (`docs/plan-implementation.md:254-256`): the
    /// synthetic fixture above proves the write engine is correct against a
    /// *known-shape* database, but never against a real Rekordbox B-tree
    /// (real page fragmentation, real row count, real column content).
    /// This is the "test against a copy of a real master.db" step the M8
    /// status notes as still outstanding before any real usage.
    ///
    /// `#[ignore]`d because it needs an out-of-repo copy that doesn't exist
    /// in CI or a fresh checkout. Run manually:
    /// `SIFT_M8_REAL_COPY_DIR=<path to a folder containing a COPY of
    /// master.db + masterPlaylists6.xml, never the live Pioneer folder>
    /// cargo test --manifest-path src-tauri/Cargo.toml -- --ignored
    /// real_masterdb_copy --nocapture`
    ///
    /// Requires Rekordbox closed (the function's own safety gate refuses
    /// otherwise, which is itself part of what this test proves).
    #[test]
    #[ignore]
    fn repair_track_path_round_trips_on_real_masterdb_copy() {
        let pioneer_dir = std::path::PathBuf::from(
            std::env::var("SIFT_M8_REAL_COPY_DIR")
                .expect("set SIFT_M8_REAL_COPY_DIR to a folder holding a COPY of master.db + masterPlaylists6.xml"),
        );
        assert!(
            pioneer_dir.join("master.db").exists(),
            "no master.db under SIFT_M8_REAL_COPY_DIR"
        );

        let tmp = tempfile::tempdir().expect("tempdir");
        let backup_dir = tmp.path().join("backup");

        let xml_before =
            std::fs::read(pioneer_dir.join("masterPlaylists6.xml")).expect("read xml before");

        let baseline =
            read_rekordbox_masterdb(&pioneer_dir.join("master.db")).expect("read real copy");
        let track_count_before = baseline.tracks.len();
        assert!(
            track_count_before > 0,
            "real copy has no tracks — wrong file?"
        );
        let target = baseline.tracks.first().expect("at least one track").clone();

        // RekordboxTrack only carries FolderPath — fetch the real
        // FileNameL/FileNameS for this row directly so the restore step at
        // the end writes back the exact original values, not a guess.
        let raw = std::fs::read(pioneer_dir.join("master.db")).expect("read real copy bytes");
        let plaintext = decrypt_masterdb(&raw).expect("decrypt real copy");
        let mut conn = Connection::open_in_memory().expect("open in-memory");
        let len = plaintext.len();
        conn.deserialize_read_exact(rusqlite::MAIN_DB, Cursor::new(plaintext), len, true)
            .expect("deserialize real copy");
        let (orig_file_name_l, orig_file_name_s): (String, String) = conn
            .query_row(
                "SELECT FileNameL, FileNameS FROM djmdContent WHERE ID = ?1",
                rusqlite::params![target.track_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("original FileNameL/FileNameS for target track");
        drop(conn);

        println!(
            "baseline: {track_count_before} tracks; repairing track_id={} original_path={}",
            target.track_id, target.folder_path
        );

        let test_path = format!("D:/SIFT_M8_TIER1_REALDATA_TEST/{}.flac", target.track_id);
        let repair = PathRepair {
            track_id: target.track_id.clone(),
            new_folder_path: test_path.clone(),
            new_file_name_l: format!("{}.flac", target.track_id),
            new_file_name_s: format!("{}.flac", target.track_id),
        };
        repair_track_path(&pioneer_dir, &backup_dir, &repair).expect("repair on real copy");

        let after_repair =
            read_rekordbox_masterdb(&pioneer_dir.join("master.db")).expect("reread after repair");
        assert_eq!(
            after_repair.tracks.len(),
            track_count_before,
            "track count changed — repair must not add/remove rows"
        );
        let repaired = after_repair
            .tracks
            .iter()
            .find(|t| t.track_id == target.track_id)
            .expect("repaired track still present");
        assert_eq!(repaired.folder_path, test_path);

        // Tier 1 deliberately never touches the XML (see repair_track_path's
        // doc comment) — assert that holds on real data too.
        let xml_after =
            std::fs::read(pioneer_dir.join("masterPlaylists6.xml")).expect("read xml after");
        assert_eq!(
            xml_before, xml_after,
            "masterPlaylists6.xml must be untouched by Tier 1"
        );

        // Restore the original path so the copy stays reusable for a rerun,
        // and to prove a second real-data write round-trips too.
        let restore = PathRepair {
            track_id: target.track_id.clone(),
            new_folder_path: target.folder_path.clone(),
            new_file_name_l: orig_file_name_l,
            new_file_name_s: orig_file_name_s,
        };
        repair_track_path(&pioneer_dir, &backup_dir, &restore)
            .expect("restore original path on real copy");

        let after_restore =
            read_rekordbox_masterdb(&pioneer_dir.join("master.db")).expect("reread after restore");
        let restored = after_restore
            .tracks
            .iter()
            .find(|t| t.track_id == target.track_id)
            .expect("restored track still present");
        assert_eq!(restored.folder_path, target.folder_path);
        assert_eq!(after_restore.tracks.len(), track_count_before);

        println!("PASS: repair + restore round-tripped cleanly on real master.db copy ({track_count_before} tracks)");
    }

    /// M8 Tier 2 real-data gate, same rationale as Tier 1's
    /// `repair_track_path_round_trips_on_real_masterdb_copy` — a synthetic
    /// fixture proves the engine's SQL is correct, not that it survives a
    /// real Rekordbox B-tree. Unlike Tier 1's test, this one does not
    /// restore the original state afterward: the real copy conveniently
    /// already has a genuine pre-existing duplicate (see
    /// `docs/ressources-externes.md`, Évaluation 18, the "Suivi même jour"
    /// paragraph appended to that section), and cleaning it up is a
    /// harmless, disposable side effect on a throwaway copy, never the
    /// live file.
    ///
    /// `#[ignore]`d for the same reason as Tier 1's — needs
    /// `SIFT_M8_REAL_COPY_DIR` and Rekordbox closed, not runnable in CI.
    #[test]
    #[ignore]
    fn dedup_playlist_group_round_trips_on_real_masterdb_copy() {
        let pioneer_dir = std::path::PathBuf::from(
            std::env::var("SIFT_M8_REAL_COPY_DIR")
                .expect("set SIFT_M8_REAL_COPY_DIR to a folder holding a COPY of master.db + masterPlaylists6.xml"),
        );
        let tmp = tempfile::tempdir().expect("tempdir");
        let backup_dir = tmp.path().join("backup");

        let groups = detect_playlist_duplicates(&pioneer_dir.join("master.db"))
            .expect("detect on real copy");
        assert!(
            !groups.is_empty(),
            "expected at least one real duplicate group to dedup"
        );
        let group = groups[0].clone();
        println!(
            "deduping playlist={} content={} keep={} remove={:?}",
            group.playlist_id,
            group.content_id,
            group.keep.song_playlist_id,
            group
                .remove
                .iter()
                .map(|e| &e.song_playlist_id)
                .collect::<Vec<_>>()
        );

        dedup_playlist_group(&pioneer_dir, &backup_dir, &group).expect("dedup on real copy");

        let after =
            detect_playlist_duplicates(&pioneer_dir.join("master.db")).expect("detect after");
        assert!(
            !after
                .iter()
                .any(|g| g.playlist_id == group.playlist_id && g.content_id == group.content_id),
            "duplicate group must be gone after dedup"
        );

        println!("PASS: deduped 1 real playlist duplicate group on a real master.db copy");
    }

    #[test]
    fn dedup_playlist_group_removes_extra_entries_and_bumps_usn() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let pioneer_dir = tmp.path().join("pioneer");
        let backup_dir = tmp.path().join("backup");
        std::fs::create_dir_all(&pioneer_dir).expect("mkdir pioneer");
        std::fs::copy(FIXTURE, pioneer_dir.join("master.db")).expect("copy fixture as master.db");
        std::fs::write(pioneer_dir.join("masterPlaylists6.xml"), b"<DJ_PLAYLISTS/>")
            .expect("write fake xml");

        let groups = detect_playlist_duplicates(&pioneer_dir.join("master.db")).expect("detect");
        assert_eq!(groups.len(), 1);
        let group = groups[0].clone();

        dedup_playlist_group(&pioneer_dir, &backup_dir, &group).expect("dedup");

        // No more duplicates for this (playlist, content) pair.
        let after =
            detect_playlist_duplicates(&pioneer_dir.join("master.db")).expect("detect after");
        assert!(!after
            .iter()
            .any(|g| g.playlist_id == group.playlist_id && g.content_id == group.content_id));

        // The kept row is still there, untouched.
        let index = read_rekordbox_masterdb(&pioneer_dir.join("master.db")).expect("reread");
        assert_eq!(
            index.tracks.len(),
            3,
            "djmdContent must be untouched by a playlist dedup"
        );

        // Open a fresh in-memory connection on the written master.db to check
        // the USN bump and the surviving `keep` row directly.
        let raw_after =
            std::fs::read(pioneer_dir.join("master.db")).expect("read written master.db");
        let plaintext_after = decrypt_masterdb(&raw_after).expect("decrypt written master.db");
        let mut conn_after = Connection::open_in_memory().expect("open in-memory");
        let len_after = plaintext_after.len();
        conn_after
            .deserialize_read_exact(
                rusqlite::MAIN_DB,
                Cursor::new(plaintext_after),
                len_after,
                false,
            )
            .expect("deserialize written master.db");

        // USN bumped by exactly 1 (the group's `remove` list has exactly 1
        // entry, fixture's agentRegistry starts at int_1 = 1000).
        let usn_after: i64 = conn_after
            .query_row(
                "SELECT int_1 FROM agentRegistry WHERE registry_id = 'localUpdateCount'",
                [],
                |row| row.get(0),
            )
            .expect("read agentRegistry.int_1 after dedup");
        assert_eq!(
            usn_after, 1001,
            "agentRegistry.int_1 must bump by 1 per removed entry"
        );

        // The `keep` row itself must still be present, with its TrackNo unchanged.
        let keep_track_no: i64 = conn_after
            .query_row(
                "SELECT TrackNo FROM djmdSongPlaylist WHERE ID = ?1",
                rusqlite::params![group.keep.song_playlist_id],
                |row| row.get(0),
            )
            .expect("keep row must survive dedup");
        assert_eq!(
            keep_track_no, group.keep.track_no,
            "kept djmdSongPlaylist row's TrackNo must be unchanged"
        );

        // Backup exists and matches the original fixture.
        let backed_up = std::fs::read(backup_dir.join("master.db")).expect("read backup");
        let original = std::fs::read(FIXTURE).expect("read fixture");
        assert_eq!(backed_up, original);
    }

    #[test]
    fn dedup_playlist_group_rejects_empty_remove_list() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let pioneer_dir = tmp.path().join("pioneer");
        let backup_dir = tmp.path().join("backup");
        std::fs::create_dir_all(&pioneer_dir).expect("mkdir pioneer");
        std::fs::copy(FIXTURE, pioneer_dir.join("master.db")).expect("copy fixture as master.db");
        std::fs::write(pioneer_dir.join("masterPlaylists6.xml"), b"<DJ_PLAYLISTS/>")
            .expect("write fake xml");

        let empty_group = PlaylistDuplicateGroup {
            playlist_id: "50000001".to_string(),
            content_id: "40000002".to_string(),
            keep: PlaylistDuplicateEntry {
                song_playlist_id: "60000002".to_string(),
                track_no: 2,
            },
            remove: vec![],
        };
        let err = dedup_playlist_group(&pioneer_dir, &backup_dir, &empty_group).unwrap_err();
        assert_eq!(err, MasterDbError::NoDuplicatesToRemove);
    }

    #[test]
    fn dedup_playlist_group_rejects_unknown_song_playlist_id() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let pioneer_dir = tmp.path().join("pioneer");
        let backup_dir = tmp.path().join("backup");
        std::fs::create_dir_all(&pioneer_dir).expect("mkdir pioneer");
        std::fs::copy(FIXTURE, pioneer_dir.join("master.db")).expect("copy fixture as master.db");
        std::fs::write(pioneer_dir.join("masterPlaylists6.xml"), b"<DJ_PLAYLISTS/>")
            .expect("write fake xml");

        let bogus_group = PlaylistDuplicateGroup {
            playlist_id: "50000001".to_string(),
            content_id: "40000001".to_string(),
            keep: PlaylistDuplicateEntry {
                song_playlist_id: "60000001".to_string(),
                track_no: 1,
            },
            remove: vec![PlaylistDuplicateEntry {
                song_playlist_id: "99999999".to_string(),
                track_no: 9,
            }],
        };
        let err = dedup_playlist_group(&pioneer_dir, &backup_dir, &bogus_group).unwrap_err();
        assert_eq!(
            err,
            MasterDbError::SongPlaylistEntryNotFound {
                song_playlist_id: "99999999".to_string()
            }
        );
    }

    fn setup_fixture_copy(tmp: &tempfile::TempDir) -> (std::path::PathBuf, std::path::PathBuf) {
        let pioneer_dir = tmp.path().join("pioneer");
        let backup_dir = tmp.path().join("backup");
        std::fs::create_dir_all(&pioneer_dir).expect("mkdir pioneer");
        std::fs::copy(FIXTURE, pioneer_dir.join("master.db")).expect("copy fixture as master.db");
        std::fs::write(pioneer_dir.join("masterPlaylists6.xml"), b"<DJ_PLAYLISTS/>")
            .expect("write fake xml");
        (pioneer_dir, backup_dir)
    }

    fn count_rows(conn: &Connection, table: &str) -> i64 {
        conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("count rows")
    }

    fn open_plain(path: &Path) -> Connection {
        let raw = std::fs::read(path).expect("read master.db");
        let plaintext = decrypt_masterdb(&raw).expect("decrypt");
        let mut conn = Connection::open_in_memory().expect("open in-memory");
        let len = plaintext.len();
        conn.deserialize_read_exact(rusqlite::MAIN_DB, Cursor::new(plaintext), len, false)
            .expect("deserialize");
        conn
    }

    // Cas 1 (plan Task 4) : nom d'artiste déjà existant dans la fixture
    // ("Existing Artist", ID 70000001) — mirroir direct du verdict spike 7
    // (REUSE) : ArtistID repointe vers la ligne existante, aucune nouvelle
    // ligne djmdArtist créée.
    #[test]
    fn sync_track_metadata_reuses_existing_artist() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (pioneer_dir, backup_dir) = setup_fixture_copy(&tmp);
        let db_path = pioneer_dir.join("master.db");

        let before = open_plain(&db_path);
        let count_before = count_rows(&before, "djmdArtist");
        drop(before);

        let sync = MetadataSync {
            track_id: "40000002".to_string(), // track with no ArtistID set yet
            artist: Some("Existing Artist".to_string()),
            ..Default::default()
        };
        sync_track_metadata(&pioneer_dir, &backup_dir, &sync).expect("sync metadata");

        let after = open_plain(&db_path);
        let count_after = count_rows(&after, "djmdArtist");
        assert_eq!(
            count_after, count_before,
            "no new djmdArtist row should be created (reuse)"
        );

        let artist_id: String = after
            .query_row(
                "SELECT ArtistID FROM djmdContent WHERE ID = '40000002'",
                [],
                |row| row.get(0),
            )
            .expect("read ArtistID");
        assert_eq!(
            artist_id, "70000001",
            "ArtistID should point at the existing row"
        );
    }

    // Cas 2 : nom d'artiste inédit — mirroir direct du verdict spike 6
    // (CREATE) : nouvelle ligne djmdArtist créée, ArtistID repointe dessus.
    #[test]
    fn sync_track_metadata_creates_new_artist() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (pioneer_dir, backup_dir) = setup_fixture_copy(&tmp);
        let db_path = pioneer_dir.join("master.db");

        let before = open_plain(&db_path);
        let count_before = count_rows(&before, "djmdArtist");
        drop(before);

        let sync = MetadataSync {
            track_id: "40000002".to_string(),
            artist: Some("Brand New Artist".to_string()),
            ..Default::default()
        };
        sync_track_metadata(&pioneer_dir, &backup_dir, &sync).expect("sync metadata");

        let after = open_plain(&db_path);
        let count_after = count_rows(&after, "djmdArtist");
        assert_eq!(
            count_after,
            count_before + 1,
            "exactly one new djmdArtist row should be created"
        );

        let (artist_id, artist_name): (String, String) = after
            .query_row(
                "SELECT c.ArtistID, a.Name FROM djmdContent c JOIN djmdArtist a ON a.ID = c.ArtistID \
                 WHERE c.ID = '40000002'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read joined artist");
        assert_eq!(artist_name, "Brand New Artist");
        assert_ne!(artist_id, "70000001");
    }

    // Cas 2b : variante casse/espaces d'un nom déjà existant — doit
    // réutiliser la ligne existante (pas de doublon cosmétique), résidu de
    // risque #1 du design fermé le 2026-07-09.
    #[test]
    fn sync_track_metadata_reuses_existing_artist_ignoring_case_and_whitespace() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (pioneer_dir, backup_dir) = setup_fixture_copy(&tmp);
        let db_path = pioneer_dir.join("master.db");

        let before = open_plain(&db_path);
        let count_before = count_rows(&before, "djmdArtist");
        drop(before);

        let sync = MetadataSync {
            track_id: "40000002".to_string(),
            artist: Some("  existing ARTIST  ".to_string()),
            ..Default::default()
        };
        sync_track_metadata(&pioneer_dir, &backup_dir, &sync).expect("sync metadata");

        let after = open_plain(&db_path);
        assert_eq!(
            count_rows(&after, "djmdArtist"),
            count_before,
            "case/whitespace variant must reuse, not duplicate"
        );
        let (artist_id, stored_name): (String, String) = after
            .query_row(
                "SELECT c.ArtistID, a.Name FROM djmdContent c JOIN djmdArtist a ON a.ID = c.ArtistID \
                 WHERE c.ID = '40000002'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read joined artist");
        assert_eq!(artist_id, "70000001", "must resolve to the existing row");
        assert_eq!(
            stored_name, "Existing Artist",
            "existing row's stored Name must be untouched"
        );
    }

    // Cas 3 : title+year seuls, aucun champ FK — écriture directe, aucune
    // table FK touchée.
    #[test]
    fn sync_track_metadata_writes_title_and_year_directly_without_touching_fk_tables() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (pioneer_dir, backup_dir) = setup_fixture_copy(&tmp);
        let db_path = pioneer_dir.join("master.db");

        let before = open_plain(&db_path);
        let (artist_before, genre_before, label_before) = (
            count_rows(&before, "djmdArtist"),
            count_rows(&before, "djmdGenre"),
            count_rows(&before, "djmdLabel"),
        );
        drop(before);

        let sync = MetadataSync {
            track_id: "40000002".to_string(),
            title: Some("Renamed Title".to_string()),
            year: Some(1999),
            ..Default::default()
        };
        sync_track_metadata(&pioneer_dir, &backup_dir, &sync).expect("sync metadata");

        let after = open_plain(&db_path);
        assert_eq!(count_rows(&after, "djmdArtist"), artist_before);
        assert_eq!(count_rows(&after, "djmdGenre"), genre_before);
        assert_eq!(count_rows(&after, "djmdLabel"), label_before);

        let (title, year): (String, i64) = after
            .query_row(
                "SELECT Title, ReleaseYear FROM djmdContent WHERE ID = '40000002'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read title/year");
        assert_eq!(title, "Renamed Title");
        assert_eq!(year, 1999);
    }

    // Cas 4 : les 5 champs en même temps sur une piste sans aucun FK
    // préexistant — vérifie le nombre exact de bumps USN globaux (3
    // créations FK + 1 pour djmdContent = 4).
    #[test]
    fn sync_track_metadata_bumps_usn_once_per_new_fk_row_plus_once_for_content() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (pioneer_dir, backup_dir) = setup_fixture_copy(&tmp);
        let db_path = pioneer_dir.join("master.db");

        let before = open_plain(&db_path);
        let usn_before: i64 = before
            .query_row(
                "SELECT int_1 FROM agentRegistry WHERE registry_id = 'localUpdateCount'",
                [],
                |row| row.get(0),
            )
            .expect("read usn before");
        drop(before);

        let sync = MetadataSync {
            track_id: "40000003".to_string(), // no ArtistID/GenreID/LabelID set in fixture
            artist: Some("Fresh Artist".to_string()),
            genre: Some("Fresh Genre".to_string()),
            label: Some("Fresh Label".to_string()),
            title: Some("Fresh Title".to_string()),
            year: Some(2020),
        };
        sync_track_metadata(&pioneer_dir, &backup_dir, &sync).expect("sync metadata");

        let after = open_plain(&db_path);
        let usn_after: i64 = after
            .query_row(
                "SELECT int_1 FROM agentRegistry WHERE registry_id = 'localUpdateCount'",
                [],
                |row| row.get(0),
            )
            .expect("read usn after");
        assert_eq!(
            usn_after,
            usn_before + 4,
            "3 new FK rows + 1 djmdContent update = 4 bumps"
        );
    }

    // Cas 5 : Analysed/AnalysisUpdated/CueUpdated inchangés — invariant M8
    // non négociable, réaffirmé pour Tier 3 comme pour les tiers précédents.
    #[test]
    fn sync_track_metadata_never_touches_analysis_columns() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (pioneer_dir, backup_dir) = setup_fixture_copy(&tmp);
        let db_path = pioneer_dir.join("master.db");

        let before = open_plain(&db_path);
        let (analysed_before, analysis_updated_before, cue_updated_before): (String, String, String) = before
            .query_row(
                "SELECT Analysed, AnalysisUpdated, CueUpdated FROM djmdContent WHERE ID = '40000001'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("read analysis columns before");
        drop(before);

        let sync = MetadataSync {
            track_id: "40000001".to_string(),
            artist: Some("Existing Artist".to_string()),
            title: Some("New Title".to_string()),
            ..Default::default()
        };
        sync_track_metadata(&pioneer_dir, &backup_dir, &sync).expect("sync metadata");

        let after = open_plain(&db_path);
        let (analysed_after, analysis_updated_after, cue_updated_after): (String, String, String) = after
            .query_row(
                "SELECT Analysed, AnalysisUpdated, CueUpdated FROM djmdContent WHERE ID = '40000001'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("read analysis columns after");
        assert_eq!(analysed_after, analysed_before);
        assert_eq!(analysis_updated_after, analysis_updated_before);
        assert_eq!(cue_updated_after, cue_updated_before);
    }

    #[test]
    fn sync_track_metadata_rejects_unknown_track_id() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (pioneer_dir, backup_dir) = setup_fixture_copy(&tmp);

        let sync = MetadataSync {
            track_id: "99999999".to_string(),
            artist: Some("Whoever".to_string()),
            ..Default::default()
        };
        let err = sync_track_metadata(&pioneer_dir, &backup_dir, &sync).unwrap_err();
        assert_eq!(
            err,
            MasterDbError::TrackNotFound {
                track_id: "99999999".to_string()
            }
        );
    }

    /// M8 Tier 3 real-data gate, same rationale as Tier 1's
    /// `repair_track_path_round_trips_on_real_masterdb_copy` (the WAL-header
    /// bug it caught, `docs/ressources-externes.md` Évaluation 18, never
    /// showed on the synthetic fixture) — proves the find-or-create engine
    /// against the real B-tree, not just the fixture's simplified schema.
    ///
    /// Exercises both branches empirically observed in M8 spikes 6/7 on the
    /// same canary (`ID=99795585`, "Street Battle" — unique title, no
    /// ambiguity possible): REUSE (repoint to a known-existing artist,
    /// "Eat Static") then CREATE (a fabricated name unique to this test
    /// run), each followed by a restore to the track's original artist —
    /// which itself exercises REUSE again, since the original artist row
    /// still exists.
    ///
    /// `#[ignore]`d — needs `SIFT_M8_REAL_COPY_DIR` (a COPY, never the live
    /// Pioneer folder) and Rekordbox closed. Run manually:
    /// `SIFT_M8_REAL_COPY_DIR=<path> cargo test --manifest-path
    /// src-tauri/Cargo.toml -- --ignored sync_track_metadata_round_trips_on_real_masterdb_copy --nocapture`
    #[test]
    #[ignore]
    fn sync_track_metadata_round_trips_on_real_masterdb_copy() {
        let pioneer_dir = std::path::PathBuf::from(
            std::env::var("SIFT_M8_REAL_COPY_DIR")
                .expect("set SIFT_M8_REAL_COPY_DIR to a folder holding a COPY of master.db + masterPlaylists6.xml"),
        );
        let db_path = pioneer_dir.join("master.db");
        assert!(db_path.exists(), "no master.db under SIFT_M8_REAL_COPY_DIR");

        let tmp = tempfile::tempdir().expect("tempdir");
        let backup_dir = tmp.path().join("backup");

        const CANARY_ID: &str = "99795585"; // "Street Battle", unique title (spikes 5/6/7)
        const KNOWN_EXISTING_ARTIST: &str = "Eat Static"; // 31 tracks in the real library (spike 7)

        let before = open_plain(&db_path);
        let title: String = before
            .query_row(
                "SELECT Title FROM djmdContent WHERE ID = ?1",
                rusqlite::params![CANARY_ID],
                |row| row.get(0),
            )
            .expect("canary present");
        assert_eq!(
            title, "Street Battle",
            "canary title mismatch — wrong copy?"
        );
        let (orig_artist_id, orig_artist_name): (String, String) = before
            .query_row(
                "SELECT c.ArtistID, a.Name FROM djmdContent c JOIN djmdArtist a ON a.ID = c.ArtistID \
                 WHERE c.ID = ?1",
                rusqlite::params![CANARY_ID],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("canary original artist");
        let artist_count_before = count_rows(&before, "djmdArtist");
        drop(before);

        println!("baseline: canary artist={orig_artist_name:?} (ID={orig_artist_id}), {artist_count_before} djmdArtist rows");

        // Phase 1: REUSE — repoint to a known-existing artist.
        let sync_reuse = MetadataSync {
            track_id: CANARY_ID.to_string(),
            artist: Some(KNOWN_EXISTING_ARTIST.to_string()),
            ..Default::default()
        };
        sync_track_metadata(&pioneer_dir, &backup_dir, &sync_reuse)
            .expect("sync to existing artist");

        let after_reuse = open_plain(&db_path);
        assert_eq!(
            count_rows(&after_reuse, "djmdArtist"),
            artist_count_before,
            "REUSE phase must not create a new djmdArtist row"
        );
        let artist_name_after_reuse: String = after_reuse
            .query_row(
                "SELECT a.Name FROM djmdContent c JOIN djmdArtist a ON a.ID = c.ArtistID WHERE c.ID = ?1",
                rusqlite::params![CANARY_ID],
                |row| row.get(0),
            )
            .expect("artist after reuse");
        assert_eq!(artist_name_after_reuse, KNOWN_EXISTING_ARTIST);
        drop(after_reuse);
        println!("PASS: REUSE phase — repointed to existing '{KNOWN_EXISTING_ARTIST}', no new row");

        // Phase 2: restore original (exercises REUSE again — original row still exists).
        let sync_restore1 = MetadataSync {
            track_id: CANARY_ID.to_string(),
            artist: Some(orig_artist_name.clone()),
            ..Default::default()
        };
        sync_track_metadata(&pioneer_dir, &backup_dir, &sync_restore1)
            .expect("restore original artist");
        let after_restore1 = open_plain(&db_path);
        let artist_id_after_restore1: String = after_restore1
            .query_row(
                "SELECT ArtistID FROM djmdContent WHERE ID = ?1",
                rusqlite::params![CANARY_ID],
                |row| row.get(0),
            )
            .expect("artist id after restore");
        assert_eq!(
            artist_id_after_restore1, orig_artist_id,
            "must repoint back to the exact original row (reuse, not a duplicate)"
        );
        assert_eq!(
            count_rows(&after_restore1, "djmdArtist"),
            artist_count_before
        );
        drop(after_restore1);
        println!("PASS: restored to original artist row via REUSE, no drift in row count");

        // Phase 3: CREATE — a name guaranteed absent from the real library.
        let fabricated_name = format!("SIFT_M8_TIER3_RUST_VERIFY_{}", std::process::id());
        let sync_create = MetadataSync {
            track_id: CANARY_ID.to_string(),
            artist: Some(fabricated_name.clone()),
            ..Default::default()
        };
        sync_track_metadata(&pioneer_dir, &backup_dir, &sync_create)
            .expect("sync to fabricated artist");
        let after_create = open_plain(&db_path);
        assert_eq!(
            count_rows(&after_create, "djmdArtist"),
            artist_count_before + 1,
            "CREATE phase must add exactly one new djmdArtist row"
        );
        drop(after_create);
        println!("PASS: CREATE phase — new row for '{fabricated_name}'");

        // Phase 4: restore original again, leaving the real copy clean for reruns
        // (the orphaned fabricated-name row is left behind, matching Rekordbox's
        // own observed behavior — spikes 6/7 — not cleaned up here either).
        let sync_restore2 = MetadataSync {
            track_id: CANARY_ID.to_string(),
            artist: Some(orig_artist_name.clone()),
            ..Default::default()
        };
        sync_track_metadata(&pioneer_dir, &backup_dir, &sync_restore2).expect("final restore");
        let after_restore2 = open_plain(&db_path);
        let artist_id_final: String = after_restore2
            .query_row(
                "SELECT ArtistID FROM djmdContent WHERE ID = ?1",
                rusqlite::params![CANARY_ID],
                |row| row.get(0),
            )
            .expect("final artist id");
        assert_eq!(artist_id_final, orig_artist_id);
        drop(after_restore2);

        println!("PASS: sync_track_metadata round-trips cleanly on real master.db copy (REUSE + CREATE both verified)");
    }

    /// Encodes a solid-color JPEG of the given size, for use as fixture
    /// artwork files and as the "new cover" input to `sync_track_artwork`.
    fn synthetic_jpeg(width: u32, height: u32, rgb: [u8; 3]) -> Vec<u8> {
        let img = image::RgbImage::from_pixel(width, height, image::Rgb(rgb));
        let mut buf = Vec::new();
        let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 90);
        encoder
            .write_image(img.as_raw(), width, height, image::ExtendedColorType::Rgb8)
            .expect("encode synthetic jpeg");
        buf
    }

    /// Seeds `pioneer_dir/share/<image_path>` and its `_m`/`_s` siblings with
    /// synthetic JPEGs of the given per-variant sizes, mirroring what a real
    /// Rekordbox artwork cache folder looks like.
    fn seed_artwork_variants(pioneer_dir: &Path, image_path: &str, sizes: [(u32, u32); 3]) {
        let (full, medium, small) =
            resolve_artwork_variants(pioneer_dir, image_path).expect("valid image_path");
        for (path, (w, h)) in [(&full, sizes[0]), (&medium, sizes[1]), (&small, sizes[2])] {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, synthetic_jpeg(w, h, [10, 20, 30])).unwrap();
        }
    }

    #[test]
    fn sync_track_artwork_resizes_new_cover_to_each_existing_variant_size() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (pioneer_dir, backup_dir) = setup_fixture_copy(&tmp);

        seed_artwork_variants(
            &pioneer_dir,
            "/PIONEER/Artwork/aaaa/artwork.jpg",
            [(500, 500), (100, 100), (40, 40)],
        );
        let new_cover = synthetic_jpeg(800, 800, [255, 0, 220]);

        sync_track_artwork(&pioneer_dir, &backup_dir, "40000001", &new_cover)
            .expect("sync should succeed");

        let (full, medium, small) =
            resolve_artwork_variants(&pioneer_dir, "/PIONEER/Artwork/aaaa/artwork.jpg")
                .expect("valid image_path");
        assert_eq!(image::image_dimensions(&full).unwrap(), (500, 500));
        assert_eq!(image::image_dimensions(&medium).unwrap(), (100, 100));
        assert_eq!(image::image_dimensions(&small).unwrap(), (40, 40));
    }

    #[test]
    fn sync_track_artwork_rejects_track_with_no_image_path() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (pioneer_dir, backup_dir) = setup_fixture_copy(&tmp);

        let err = sync_track_artwork(
            &pioneer_dir,
            &backup_dir,
            "40000002",
            &synthetic_jpeg(10, 10, [0, 0, 0]),
        )
        .unwrap_err();
        assert_eq!(
            err,
            MasterDbError::NoArtworkPath {
                track_id: "40000002".to_string()
            }
        );
    }

    #[test]
    fn sync_track_artwork_rejects_when_a_variant_file_is_missing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (pioneer_dir, backup_dir) = setup_fixture_copy(&tmp);

        // Track 40000003 has an ImagePath in the fixture but no files were
        // ever seeded on disk for it.
        let err = sync_track_artwork(
            &pioneer_dir,
            &backup_dir,
            "40000003",
            &synthetic_jpeg(10, 10, [0, 0, 0]),
        )
        .unwrap_err();
        assert!(matches!(err, MasterDbError::ArtworkVariantMissing { .. }));
    }

    #[test]
    fn resolve_artwork_variants_rejects_path_traversal() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let pioneer_dir = tmp.path();

        let err = resolve_artwork_variants(pioneer_dir, "../../../../outside.jpg").unwrap_err();
        assert!(matches!(err, MasterDbError::ArtworkPathEscapesRoot { .. }));

        let err = resolve_artwork_variants(pioneer_dir, "/PIONEER/../../outside.jpg").unwrap_err();
        assert!(matches!(err, MasterDbError::ArtworkPathEscapesRoot { .. }));

        let err = resolve_artwork_variants(pioneer_dir, r"C:\Windows\outside.jpg").unwrap_err();
        assert!(matches!(err, MasterDbError::ArtworkPathEscapesRoot { .. }));
    }

    #[test]
    fn sync_track_artwork_never_touches_masterdb_bytes() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (pioneer_dir, backup_dir) = setup_fixture_copy(&tmp);

        seed_artwork_variants(
            &pioneer_dir,
            "/PIONEER/Artwork/aaaa/artwork.jpg",
            [(200, 200), (80, 80), (30, 30)],
        );
        let before = std::fs::read(pioneer_dir.join("master.db")).unwrap();

        sync_track_artwork(
            &pioneer_dir,
            &backup_dir,
            "40000001",
            &synthetic_jpeg(50, 50, [1, 2, 3]),
        )
        .expect("sync should succeed");

        let after = std::fs::read(pioneer_dir.join("master.db")).unwrap();
        assert_eq!(before, after, "master.db must be byte-for-byte unchanged");
    }

    /// Manual-only: exercises sync_track_artwork against a fresh copy of a
    /// real master.db + its real artwork cache folder (Rekordbox closed).
    /// Same canary as the sync_track_metadata real-copy test
    /// (`ID=99795585`, "Street Battle") — `ImagePath` confirmed present at
    /// spike 8. Run manually:
    /// `SIFT_M8_REAL_COPY_DIR=<dir with master.db + share/ + masterPlaylists6.xml>
    /// cargo test --manifest-path src-tauri/Cargo.toml sync_track_artwork_round_trips_on_real_masterdb_copy -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn sync_track_artwork_round_trips_on_real_masterdb_copy() {
        let Ok(dir) = std::env::var("SIFT_M8_REAL_COPY_DIR") else {
            eprintln!("skip: set SIFT_M8_REAL_COPY_DIR to run this test");
            return;
        };
        let pioneer_dir = Path::new(&dir);
        let backup_dir = pioneer_dir.join("sift-artwork-test-backup");
        let canary_id = "99795585";

        let (full, medium, small) = resolve_artwork_variants(
            pioneer_dir,
            "/PIONEER/Artwork/873/2140a-d8c1-472c-991b-6b281cf6005f/artwork.jpg",
        )
        .expect("valid image_path");
        let before_full = std::fs::read(&full).expect("read baseline full artwork");
        let before_medium = std::fs::read(&medium).expect("read baseline medium artwork");
        let before_small = std::fs::read(&small).expect("read baseline small artwork");

        let test_cover = synthetic_jpeg(600, 600, [12, 200, 90]);
        sync_track_artwork(pioneer_dir, &backup_dir, canary_id, &test_cover)
            .expect("sync should succeed on real copy");

        let after_full = std::fs::read(&full).unwrap();
        assert_ne!(before_full, after_full, "full artwork should have changed");

        // Restore the original artwork files independently of the
        // function's own backup mechanism, so this test leaves the real
        // copy exactly as it found it.
        std::fs::write(&full, &before_full).unwrap();
        std::fs::write(&medium, &before_medium).unwrap();
        std::fs::write(&small, &before_small).unwrap();
        let restored_full = std::fs::read(&full).unwrap();
        assert_eq!(
            restored_full, before_full,
            "restore must be verified independently"
        );
        println!(
            "PASS: sync_track_artwork round-trips cleanly on real master.db copy, artwork restored"
        );
    }

    /// Mesure du coût fixe d'une ouverture de `master.db` : la dérivation de clés SQLCipher v4,
    /// soit `KDF_ITER` = 256 000 itérations PBKDF2-HMAC-SHA512, plus les 2 itérations du HMAC.
    ///
    /// Ce chiffre est la MESURE PRÉALABLE exigée par le PRD perf-fixes §5.3 avant d'engager la
    /// tranche « un cycle master.db par lot au lieu d'un par ligne » : le gain de la tranche vaut
    /// exactement ce coût multiplié par le nombre de lignes évitées, et cette tranche est la seule
    /// qui change la sémantique transactionnelle d'écriture dans le fichier d'une application
    /// tierce — elle ne se paie que si le chiffre la justifie.
    ///
    /// N'ouvre AUCUN fichier : la dérivation ne dépend que de la passphrase et du sel, jamais de la
    /// taille de la base. Aucun contact avec un `master.db` réel, donc rien à protéger ici.
    ///
    /// `#[ignore]` et à lancer EN RELEASE — en debug, PBKDF2 est dominé par le coût du profil de
    /// compilation et le chiffre ne veut rien dire :
    /// `cargo test --release --manifest-path src-tauri/Cargo.toml -- --ignored --nocapture bench_derive_keys`
    #[ignore = "bench a la demande, a lancer en --release"]
    #[test]
    fn bench_derive_keys_cout_fixe_par_ouverture() {
        let passphrase = deobfuscate_key().expect("passphrase");
        let salt = [0x42u8; SALT_LEN];

        // Un tour à blanc : il absorbe le premier remplissage de cache et la mise en route du
        // prédicteur, qui feraient porter à la première mesure un coût qu'aucune ouverture réelle
        // ne paie deux fois.
        let _ = derive_keys(&passphrase, &salt);

        const RUNS: u32 = 5;
        let mut total = std::time::Duration::ZERO;
        let mut worst = std::time::Duration::ZERO;
        for _ in 0..RUNS {
            let t0 = std::time::Instant::now();
            let _ = derive_keys(&passphrase, &salt);
            let dt = t0.elapsed();
            total += dt;
            worst = worst.max(dt);
        }

        println!(
            "derive_keys: {} itérations PBKDF2-HMAC-SHA512 (+{} HMAC) | moyenne {:?} sur {RUNS} tours | pire {:?}",
            KDF_ITER, HMAC_KDF_ITER, total / RUNS, worst
        );
    }
}
