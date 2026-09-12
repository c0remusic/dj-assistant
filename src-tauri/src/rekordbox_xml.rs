//! Rekordbox `DJ_PLAYLISTS` XML: parse into an in-memory tree, merge Sift's filed tracks in,
//! patch one track's `Location` in place, and rewrite. Every mutation Sift makes — `patch_location`
//! AND `merge_filed_tracks` — is applied to `raw_xml` as TARGETED TEXT SURGERY (new `<TRACK>` rows
//! and playlist `<NODE>` entries inserted next to the existing text, attribute values replaced in
//! place), never by re-serializing a fresh document from a structured model. Every byte Sift
//! doesn't understand (ratings, tonality, custom columns, smart-playlist `Type="4"` nodes) survives
//! untouched, on every track in the file, not just the ones Sift's own filing/merge touched — a
//! full serde/struct round-trip would silently drop everything this module never modeled. The
//! `collection`/`playlists`/`path_index` structured view exists ALONGSIDE `raw_xml` (kept in sync
//! on every mutation) purely to make merge/lookup DECISIONS (is this track already here? does this
//! playlist already exist?) — it is never itself serialized back out. `write` simply returns the
//! already-up-to-date `raw_xml`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A small, hand-built `DJ_PLAYLISTS` XML used by tests across this module, `ipc_library.rs`,
/// and `actions.rs` — 3 collection tracks, nested folders, and one `TrackID` (2) shared by two
/// playlists (`House` + `Favorites`). Embedded as a constant rather than an external fixture
/// file so tests don't depend on `src-tauri/fixtures/` contents (that directory is gitignored —
/// see its README — and only guaranteed to contain regenerable/optional audio anchors, not this).
#[cfg(test)]
pub(crate) const SAMPLE_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>

<DJ_PLAYLISTS Version="1.0.0">
  <PRODUCT Name="rekordbox" Version="6.7.7" Company="Pioneer DJ"/>
  <COLLECTION Entries="3">
    <TRACK TrackID="1" Name="Can You Feel It" Artist="Mr Fingers" Location="file://localhost/C:/Music/House/mr-fingers.mp3"/>
    <TRACK TrackID="2" Name="Strings of Life" Artist="Rhythim Is Rhythim" Location="file://localhost/C:/Music/House/deep/strings.aiff"/>
    <TRACK TrackID="3" Name="Voodoo Ray" Artist="A Guy Called Gerald" Location="file://localhost/C:/Music/Techno/voodoo.flac"/>
  </COLLECTION>
  <PLAYLISTS>
    <NODE Type="0" Name="ROOT" Count="2">
      <NODE Type="1" Name="House" KeyType="0" Entries="2">
        <TRACK Key="1"/>
        <TRACK Key="2"/>
      </NODE>
      <NODE Type="1" Name="Favorites" KeyType="0" Entries="1">
        <TRACK Key="2"/>
      </NODE>
    </NODE>
  </PLAYLISTS>
</DJ_PLAYLISTS>
"#;

/// One `<TRACK>` row under `<COLLECTION>`.
#[derive(Debug, Clone, PartialEq)]
pub struct CollectionTrack {
    pub track_id: i64,
    pub location: String,
    pub name: Option<String>,
    pub artist: Option<String>,
}

/// One node of the `<PLAYLISTS>` tree: either a folder (nested nodes) or a leaf playlist
/// (an ordered list of `TrackID`s, mirroring `<TRACK Key="...">` children).
#[derive(Debug, Clone, PartialEq)]
pub enum PlaylistNode {
    Folder {
        name: String,
        children: Vec<PlaylistNode>,
    },
    Playlist {
        name: String,
        track_ids: Vec<i64>,
    },
}

/// A parsed Rekordbox XML: structured view for merge/lookup decisions, plus `raw_xml` — the
/// document text, kept byte-preserving and up to date by BOTH `patch_location` and
/// `merge_filed_tracks` via targeted text surgery (see the module doc comment).
#[derive(Debug, Clone)]
pub struct RekordboxXml {
    pub collection: Vec<CollectionTrack>,
    pub playlists: Vec<PlaylistNode>,
    pub raw_xml: String,
    /// Normalized filesystem path → TrackID, built once at parse time for O(1) lookups.
    path_index: HashMap<PathBuf, i64>,
}

impl RekordboxXml {
    /// Look up the `TrackID` for a filesystem path (normalized the same way `Location` is),
    /// or `None` if this XML doesn't reference that path. `path` is re-normalized for the
    /// case-insensitive lookup key (see `path_index_key`) so a caller passing a path that
    /// differs from the stored `Location` only by drive-letter/segment casing (Sift's own
    /// scanner, or a user-typed path) still matches — Sift targets Windows/macOS, both
    /// case-insensitive-preserving filesystems by default.
    pub fn track_id_for_path(&self, path: &Path) -> Option<i64> {
        self.path_index.get(&path_index_key(path)).copied()
    }
}

/// Rekordbox stores `Location` as a `file://localhost/`-prefixed, percent-encoded URI
/// (e.g. `file://localhost/C:/Music/House/a.mp3`). Strip the prefix and percent-decode so it
/// compares equal to the plain filesystem paths Sift's `actions.from_path`/`to_path` use.
fn normalize_path(location: &str) -> PathBuf {
    let stripped = location
        .strip_prefix("file://localhost/")
        .or_else(|| location.strip_prefix("file://"))
        .unwrap_or(location);
    let decoded = percent_decode(stripped);
    PathBuf::from(decoded)
}

/// FIX-6: the `path_index` HashMap key, additionally lowercased on top of `normalize_path`'s
/// separator/percent-decoding. A plain `PathBuf` equality (what the index used before this fix)
/// is case-SENSITIVE, so a drive-letter or segment casing difference between the XML's `Location`
/// and the path a caller looks up with (e.g. Sift rewrote a filing to `House/Deep/x.aiff` while
/// the XML still had `house/deep/x.aiff`) silently missed the lookup — the exact scenario
/// `patch_location`/`merge_filed_tracks` exist to catch. Both Windows and macOS default to
/// case-insensitive-preserving filesystems (Sift's only two targets), so lowercasing the whole
/// path for the comparison key is correct on both, not just the drive letter. The ORIGINAL casing
/// is still preserved in `CollectionTrack.location`/the written XML — only this lookup key is
/// case-folded.
fn path_index_key(path: &Path) -> PathBuf {
    PathBuf::from(path.to_string_lossy().to_lowercase())
}

/// Minimal percent-decoder for the subset Rekordbox actually emits (`%20`, `%23`, etc.) — no
/// external dependency needed for this one narrow job.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Parse raw Rekordbox XML bytes into a `RekordboxXml`. Fails fast on malformed XML or a
/// missing `<DJ_PLAYLISTS>` root — no partial/best-effort tree is ever returned.
pub fn parse(xml_bytes: &[u8]) -> Result<RekordboxXml, String> {
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;

    let raw_xml = String::from_utf8_lossy(xml_bytes).into_owned();
    let mut reader = Reader::from_str(&raw_xml);
    reader.config_mut().trim_text(true);

    let mut collection = Vec::new();
    // Stack frame: (is_folder, name, children-collected-so-far).
    let mut stack: Vec<(bool, String, Vec<PlaylistNode>)> = Vec::new();
    let mut playlists = Vec::new();
    let mut saw_root_tag = false;
    let mut in_playlists = false;

    loop {
        let event = reader
            .read_event()
            .map_err(|e| format!("Rekordbox XML parse error: {e}"))?;
        match event {
            Event::Eof => break,
            Event::Start(e) => {
                let tag = e.name();
                match tag.as_ref() {
                    b"DJ_PLAYLISTS" => saw_root_tag = true,
                    b"PLAYLISTS" => in_playlists = true,
                    b"TRACK" if !in_playlists => {
                        collection.push(collection_track_from_attrs(&e)?);
                    }
                    b"TRACK" if in_playlists => {
                        push_playlist_key(&mut stack, &e)?;
                    }
                    b"NODE" if in_playlists => {
                        let attrs = read_attrs(&e)?;
                        let name = attrs.get("Name").cloned().unwrap_or_default();
                        let is_folder = attrs.get("Type").map(String::as_str) == Some("0");
                        stack.push((is_folder, name, Vec::new()));
                    }
                    _ => {}
                }
            }
            Event::Empty(e) => {
                let tag = e.name();
                match tag.as_ref() {
                    b"TRACK" if !in_playlists => collection.push(collection_track_from_attrs(&e)?),
                    b"TRACK" if in_playlists => {
                        push_playlist_key(&mut stack, &e)?;
                    }
                    b"NODE" if in_playlists => {
                        // Empty leaf playlist (no <TRACK> children) — push directly, no push/pop.
                        let attrs = read_attrs(&e)?;
                        let name = attrs.get("Name").cloned().unwrap_or_default();
                        push_child(
                            &mut stack,
                            &mut playlists,
                            PlaylistNode::Playlist {
                                name,
                                track_ids: Vec::new(),
                            },
                        );
                    }
                    _ => {}
                }
            }
            Event::End(e) => match e.name().as_ref() {
                b"PLAYLISTS" => in_playlists = false,
                b"NODE" => {
                    let (is_folder, name, children) = stack.pop().ok_or("unbalanced </NODE>")?;
                    let node = if is_folder {
                        PlaylistNode::Folder { name, children }
                    } else {
                        // A leaf <NODE> was entered via Start: its <TRACK Key> children were
                        // folded onto a placeholder Playlist pushed by push_playlist_key.
                        let track_ids = children
                            .into_iter()
                            .filter_map(|c| match c {
                                PlaylistNode::Playlist { track_ids, .. } => Some(track_ids),
                                _ => None,
                            })
                            .next()
                            .unwrap_or_default();
                        PlaylistNode::Playlist { name, track_ids }
                    };
                    push_child(&mut stack, &mut playlists, node);
                }
                _ => {}
            },
            _ => {}
        }
    }

    if !saw_root_tag {
        return Err("missing <DJ_PLAYLISTS> root element".to_string());
    }

    let mut path_index = HashMap::new();
    for t in &collection {
        path_index.insert(path_index_key(&normalize_path(&t.location)), t.track_id);
    }

    Ok(RekordboxXml {
        collection,
        playlists,
        raw_xml,
        path_index,
    })
}

/// Handle a `<TRACK Key="...">` inside `<PLAYLISTS>`: fold it onto the current leaf playlist's
/// `track_ids`, creating a placeholder `Playlist` child on first use so subsequent keys append
/// to the same node (closed out into its final form in the `Event::End(NODE)` handler above).
fn push_playlist_key(
    stack: &mut [(bool, String, Vec<PlaylistNode>)],
    e: &quick_xml::events::BytesStart,
) -> Result<(), String> {
    let attrs = read_attrs(e)?;
    let key: i64 = attrs
        .get("Key")
        .ok_or("playlist <TRACK> missing Key")?
        .parse()
        .map_err(|_| "bad playlist Key".to_string())?;
    if let Some((_, name, children)) = stack.last_mut() {
        if let Some(PlaylistNode::Playlist { track_ids, .. }) = children.last_mut() {
            track_ids.push(key);
        } else {
            children.push(PlaylistNode::Playlist {
                name: name.clone(),
                track_ids: vec![key],
            });
        }
    }
    Ok(())
}

/// Push a finished child node either onto the new top-of-stack frame's children, or (stack now
/// empty) onto the top-level `playlists` list.
fn push_child(
    stack: &mut [(bool, String, Vec<PlaylistNode>)],
    playlists: &mut Vec<PlaylistNode>,
    node: PlaylistNode,
) {
    if let Some((_, _, children)) = stack.last_mut() {
        children.push(node);
    } else {
        playlists.push(node);
    }
}

fn collection_track_from_attrs(
    e: &quick_xml::events::BytesStart,
) -> Result<CollectionTrack, String> {
    let attrs = read_attrs(e)?;
    let track_id: i64 = attrs
        .get("TrackID")
        .ok_or("<TRACK> missing TrackID")?
        .parse()
        .map_err(|_| "bad TrackID".to_string())?;
    Ok(CollectionTrack {
        track_id,
        location: attrs.get("Location").cloned().unwrap_or_default(),
        name: attrs.get("Name").cloned(),
        artist: attrs.get("Artist").cloned(),
    })
}

/// Read every attribute of a start/empty tag into a plain map, XML-unescaped.
fn read_attrs(e: &quick_xml::events::BytesStart) -> Result<HashMap<String, String>, String> {
    let mut out = HashMap::new();
    for attr in e.attributes() {
        let attr = attr.map_err(|err| format!("bad attribute: {err}"))?;
        let key = String::from_utf8_lossy(attr.key.as_ref()).into_owned();
        let value = attr
            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
            .map_err(|err| format!("bad attribute value: {err}"))?
            .into_owned();
        out.insert(key, value);
    }
    Ok(out)
}

/// Add every `filed` track absent from `xml.collection` (matched by normalized path), and file
/// each newly-added track into a per-`folder` playlist under the top-level playlist forest —
/// nested `folder` paths (e.g. "House/Deep") become nested playlist folders. TrackIDs are
/// allocated as `max(existing) + 1`. Existing playlists (Sift-managed or not) are never removed
/// or reordered; a folder playlist that already exists just gets the new TrackID appended.
///
/// FIX-2: mirrors every mutation into `xml.raw_xml` via targeted text surgery (new `<TRACK>` rows
/// inserted just before `</COLLECTION>`, new/updated playlist `<NODE>`s inserted/patched in place)
/// instead of only updating the structured `collection`/`playlists` view — `write()` used to
/// re-serialize from that structured view alone, silently dropping every attribute this module
/// never modeled (Rating, BPM, cue points, smart-playlist `Type="4"` nodes, custom columns) on
/// EVERY export. See `write()`'s doc comment for why raw_xml is now the sole source of truth for
/// what gets written.
///
/// Returns the number of newly-added collection tracks.
pub fn merge_filed_tracks(xml: &mut RekordboxXml, filed: &[crate::library::LibraryTrack]) -> usize {
    let mut next_id = xml.collection.iter().map(|t| t.track_id).max().unwrap_or(0) + 1;
    let mut added = 0usize;

    for track in filed {
        let norm = normalize_path(&track.path);
        let key = path_index_key(&norm);
        if xml.path_index.contains_key(&key) {
            continue; // already tracked — merge is idempotent by design
        }
        let track_id = next_id;
        next_id += 1;
        added += 1;

        let location = format!("file://localhost/{}", encode_location_path(&track.path));
        let name = track.title.clone();
        let artist = track.artist.clone();

        insert_collection_track_raw(
            &mut xml.raw_xml,
            track_id,
            name.as_deref(),
            artist.as_deref(),
            &location,
        );

        xml.collection.push(CollectionTrack {
            track_id,
            location,
            name,
            artist,
        });
        xml.path_index.insert(key, track_id);

        if let Some(folder) = &track.folder {
            let root_children = root_folder_children(&mut xml.playlists);
            file_into_folder_playlist(root_children, folder, track_id);
            insert_playlist_key_raw(&mut xml.raw_xml, folder, track_id);
        }
    }
    added
}

/// FIX-2: insert one new `<TRACK .../>` row into `raw_xml`'s `<COLLECTION>`, just before its
/// closing tag, and bump the `Entries="N"` count on the `<COLLECTION>` opening tag. Every other
/// byte of the file (including every unmodeled attribute on every OTHER `<TRACK>`) is untouched —
/// this never re-serializes an existing row, only appends a brand-new one.
fn insert_collection_track_raw(
    raw_xml: &mut String,
    track_id: i64,
    name: Option<&str>,
    artist: Option<&str>,
    location: &str,
) {
    let new_row = format!(
        "    <TRACK TrackID=\"{}\" Name=\"{}\" Artist=\"{}\" Location=\"{}\"/>\n",
        track_id,
        xml_escape(name.unwrap_or("")),
        xml_escape(artist.unwrap_or("")),
        xml_escape(location),
    );
    insert_before_closing_tag(raw_xml, "</COLLECTION>", &new_row);
    bump_count_attr(raw_xml, "<COLLECTION", "Entries");
}

/// FIX-2: append one `<TRACK Key="..."/>` row inside the named leaf playlist's `<NODE>` (matched
/// by its `Name="folder"` attribute — folder is the LAST segment only; nested paths are handled
/// by `file_into_folder_playlist` already having created/found the right leaf before this is
/// called), just before that `<NODE>`'s own closing tag, and bump its `Entries="N"` count. If no
/// `<NODE Name="folder">` exists yet in raw_xml (a brand-new playlist `file_into_folder_playlist`
/// just created in the structured tree), insert a whole new leaf `<NODE>` block instead, just
/// before the ROOT folder's closing `</NODE>` (or before `</PLAYLISTS>` if there's no ROOT node
/// in the text at all — a from-scratch tree).
fn insert_playlist_key_raw(raw_xml: &mut String, folder: &str, track_id: i64) {
    let leaf_name = folder.rsplit('/').next().unwrap_or(folder);
    let node_open_needle = format!("Name=\"{}\"", xml_escape(leaf_name));

    // Find a NODE opening tag whose Name attribute matches the leaf playlist name AND that is a
    // leaf (Type="1"), by scanning for `<NODE Type="1" ... Name="leaf_name" ...>` on one line —
    // every NODE this module inserts (see insert_playlist_key_raw below) and every NODE real
    // Rekordbox exports emit is single-line for its opening tag.
    let existing_leaf_line: Option<String> = raw_xml
        .lines()
        .find(|l| l.contains("<NODE") && l.contains("Type=\"1\"") && l.contains(&node_open_needle))
        .map(str::to_string);

    if let Some(line) = existing_leaf_line {
        if line.trim_end().ends_with("/>") {
            // Previously-empty leaf playlist (self-closing `<NODE .../>`) — expand it into an
            // open/close pair with the one new TRACK child, preserving every other attribute.
            let opening = line.trim_end().trim_end_matches("/>").to_string() + ">";
            let indent = leading_whitespace(&line);
            let new_block =
                format!("{opening}\n{indent}  <TRACK Key=\"{track_id}\"/>\n{indent}</NODE>",);
            *raw_xml = raw_xml.replacen(line.trim_end(), &new_block, 1);
            bump_count_attr_on_line(raw_xml, &node_open_needle, "Entries");
        } else {
            // Already an open/close block with existing <TRACK> children — insert the new key
            // just before this NODE's OWN closing tag (the nearest `</NODE>` after this line that
            // closes THIS node, not a nested child) and bump its Entries count.
            insert_before_matching_node_close(raw_xml, &line, track_id);
            bump_count_attr_on_line(raw_xml, &node_open_needle, "Entries");
        }
        return;
    }

    // No existing leaf NODE in raw_xml for this name — a brand-new playlist. Insert a fresh leaf
    // NODE block just before the ROOT folder's closing </NODE>, or before </PLAYLISTS> if there's
    // no ROOT node at all in the text.
    let new_node = format!(
        "      <NODE Type=\"1\" Name=\"{}\" KeyType=\"0\" Entries=\"1\">\n        <TRACK Key=\"{track_id}\"/>\n      </NODE>\n",
        xml_escape(leaf_name)
    );
    if raw_xml.contains("Name=\"ROOT\"") {
        insert_before_last(raw_xml, "</NODE>", &new_node);
    } else {
        insert_before_closing_tag(raw_xml, "</PLAYLISTS>", &new_node);
    }
}

/// The leading whitespace (indentation) of a line, for re-indenting an inserted sibling line.
fn leading_whitespace(line: &str) -> String {
    line.chars().take_while(|c| c.is_whitespace()).collect()
}

/// Insert `insertion` immediately before the FIRST occurrence of `closing_tag` in `raw_xml`.
fn insert_before_closing_tag(raw_xml: &mut String, closing_tag: &str, insertion: &str) {
    if let Some(pos) = raw_xml.find(closing_tag) {
        raw_xml.insert_str(pos, insertion);
    }
}

/// Insert `insertion` immediately before the LAST occurrence of `needle` in `raw_xml` — used for
/// the ROOT folder's closing `</NODE>`, which (being the outermost playlist folder) is always the
/// last `</NODE>` in the document.
fn insert_before_last(raw_xml: &mut String, needle: &str, insertion: &str) {
    if let Some(pos) = raw_xml.rfind(needle) {
        raw_xml.insert_str(pos, insertion);
    }
}

/// Insert a new `<TRACK Key="id"/>` line right before the closing `</NODE>` that matches
/// `opening_line` (the leaf playlist's own `<NODE ...>` opening line) — i.e. the FIRST `</NODE>`
/// found after `opening_line` in the text, since a leaf playlist NODE never nests further NODEs.
fn insert_before_matching_node_close(raw_xml: &mut String, opening_line: &str, track_id: i64) {
    let Some(open_pos) = raw_xml.find(opening_line) else {
        return;
    };
    let search_from = open_pos + opening_line.len();
    let Some(rel_close) = raw_xml[search_from..].find("</NODE>") else {
        return;
    };
    let close_pos = search_from + rel_close;
    let indent = leading_whitespace(opening_line);
    let new_line = format!("{indent}  <TRACK Key=\"{track_id}\"/>\n");
    raw_xml.insert_str(close_pos, &new_line);
}

/// Bump the integer value of `attr="N"` (e.g. `Entries="2"` → `Entries="3"`) on the tag whose
/// opening substring is `tag_prefix` (e.g. `"<COLLECTION"`) — used for `<COLLECTION Entries=...>`,
/// which is unique in the document.
fn bump_count_attr(raw_xml: &mut String, tag_prefix: &str, attr: &str) {
    let Some(tag_start) = raw_xml.find(tag_prefix) else {
        return;
    };
    let Some(tag_end_rel) = raw_xml[tag_start..].find('>') else {
        return;
    };
    let tag_end = tag_start + tag_end_rel;
    bump_count_attr_in_range(raw_xml, tag_start, tag_end, attr);
}

/// Like `bump_count_attr`, but locates the tag by a substring known to appear WITHIN it (e.g. a
/// `Name="..."` attribute) rather than by its leading prefix — used for a specific playlist
/// `<NODE>` picked out earlier by `insert_playlist_key_raw`.
fn bump_count_attr_on_line(raw_xml: &mut String, contains_needle: &str, attr: &str) {
    let Some(needle_pos) = raw_xml.find(contains_needle) else {
        return;
    };
    // Walk back to this tag's `<` and forward to its next `>` (self-closing `/>` or plain `>`).
    let tag_start = raw_xml[..needle_pos].rfind('<').unwrap_or(needle_pos);
    let Some(tag_end_rel) = raw_xml[tag_start..].find('>') else {
        return;
    };
    let tag_end = tag_start + tag_end_rel;
    bump_count_attr_in_range(raw_xml, tag_start, tag_end, attr);
}

fn bump_count_attr_in_range(raw_xml: &mut String, tag_start: usize, tag_end: usize, attr: &str) {
    let needle = format!(r#"{attr}=""#);
    let Some(rel) = raw_xml[tag_start..tag_end].find(&needle) else {
        return;
    };
    let val_start = tag_start + rel + needle.len();
    let Some(rel_end) = raw_xml[val_start..tag_end].find('"') else {
        return;
    };
    let val_end = val_start + rel_end;
    if let Ok(n) = raw_xml[val_start..val_end].parse::<i64>() {
        raw_xml.replace_range(val_start..val_end, &(n + 1).to_string());
    }
}

/// Rekordbox always nests real playlists under one top-level "ROOT" folder node (see the
/// fixture: `<NODE Type="0" Name="ROOT">`). Return a mutable reference to that folder's
/// `children`, creating the ROOT node if this XML doesn't have one yet (e.g. a brand-new tree),
/// so Sift's per-folder playlists always land inside it rather than as bare top-level siblings.
fn root_folder_children(playlists: &mut Vec<PlaylistNode>) -> &mut Vec<PlaylistNode> {
    let idx = playlists
        .iter()
        .position(|n| matches!(n, PlaylistNode::Folder { name, .. } if name == "ROOT"));
    let idx = idx.unwrap_or_else(|| {
        playlists.push(PlaylistNode::Folder {
            name: "ROOT".to_string(),
            children: Vec::new(),
        });
        playlists.len() - 1
    });
    match &mut playlists[idx] {
        PlaylistNode::Folder { children, .. } => children,
        // INVARIANT: `idx` was just found via `position()` filtered on `PlaylistNode::Folder`,
        // or freshly pushed as a `Folder` two lines above — never a `Playlist` at this index.
        PlaylistNode::Playlist { .. } => unreachable!("ROOT is always matched as a Folder above"),
    }
}

/// Ensure a (possibly nested, "/"-separated) playlist folder path exists under the top-level
/// `playlists` forest, creating folders/leaf playlists as needed, then append `track_id` to the
/// leaf playlist — unless it's already there (idempotent). Mirrors `folder`'s nesting 1:1: each
/// path segment except the last becomes/reuses a `Folder`, the last becomes/reuses a `Playlist`.
fn file_into_folder_playlist(playlists: &mut Vec<PlaylistNode>, folder: &str, track_id: i64) {
    let segments: Vec<&str> = folder.split('/').filter(|s| !s.is_empty()).collect();
    if segments.is_empty() {
        return;
    }
    file_into_level(playlists, &segments, track_id);
}

/// Recursive step of `file_into_folder_playlist`: `segments` is the remaining (non-empty) path
/// under `level`. Written as a fresh recursive call per segment (rather than a loop that
/// reassigns a `&mut` binding to a nested field of itself) so each level's mutable borrow of
/// `level` ends before the next recursive call starts — sidesteps the borrow checker rejecting
/// a self-referential "narrow the slice, keep going" loop over `Vec<PlaylistNode>`.
fn file_into_level(level: &mut Vec<PlaylistNode>, segments: &[&str], track_id: i64) {
    let (seg, rest) = match segments.split_first() {
        Some(pair) => pair,
        None => return,
    };
    let is_last = rest.is_empty();
    let idx = level.iter().position(|n| match n {
        PlaylistNode::Folder { name, .. } if !is_last => name == seg,
        PlaylistNode::Playlist { name, .. } if is_last => name == seg,
        _ => false,
    });
    match idx {
        Some(pos) => {
            if is_last {
                if let PlaylistNode::Playlist { track_ids, .. } = &mut level[pos] {
                    if !track_ids.contains(&track_id) {
                        track_ids.push(track_id);
                    }
                }
            } else if let PlaylistNode::Folder { children, .. } = &mut level[pos] {
                file_into_level(children, rest, track_id);
            }
        }
        None => {
            if is_last {
                level.push(PlaylistNode::Playlist {
                    name: (*seg).to_string(),
                    track_ids: vec![track_id],
                });
            } else {
                level.push(PlaylistNode::Folder {
                    name: (*seg).to_string(),
                    children: Vec::new(),
                });
                let last = level.len() - 1;
                if let PlaylistNode::Folder { children, .. } = &mut level[last] {
                    file_into_level(children, rest, track_id);
                }
            }
        }
    }
}

/// Outcome of `patch_location`. FIX-7: distinguishes the ordinary "this path isn't tracked at all"
/// no-op from the genuinely alarming "it's tracked, but the raw text is ambiguous" case — a caller
/// (namely `actions::repair_rekordbox_xml_if_linked`) needs to tell them apart to surface real
/// drift to the user instead of only ever logging it server-side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchLocationResult {
    /// The Location was found and rewritten.
    Patched,
    /// `from_path` isn't referenced by this XML at all — ordinary, not an error (e.g. a file
    /// filed before this XML was ever linked).
    NotTracked,
    /// `from_path` IS tracked, but the exact `Location="..."` text expected in `raw_xml` occurs
    /// zero or more-than-one times — the raw text and the structured model have drifted (e.g. two
    /// tracks sharing byte-identical Location, or a prior mutation not reflected in raw_xml).
    /// Refuses to guess which occurrence to touch; nothing is changed.
    Drifted,
}

/// Rewrite one `TrackID`'s `Location` in place: in `raw_xml` (a targeted string replace of just
/// that attribute's value, so every other byte of the file — including fields this module never
/// modeled — survives untouched), and mirrored in the structured `collection`/`path_index` so
/// subsequent `merge_filed_tracks`/`track_id_for_path` calls see the new path immediately.
pub fn patch_location(
    xml: &mut RekordboxXml,
    from_path: &str,
    to_path: &str,
) -> PatchLocationResult {
    let from_norm = normalize_path(from_path);
    let Some(track_id) = xml.track_id_for_path(&from_norm) else {
        return PatchLocationResult::NotTracked;
    };
    let Some(track) = xml.collection.iter_mut().find(|t| t.track_id == track_id) else {
        return PatchLocationResult::NotTracked; // index/collection out of sync — never guess
    };

    let old_location_attr = format!(r#"Location="{}""#, xml_escape(&track.location));
    let new_location_value = format!("file://localhost/{}", encode_location_path(to_path));
    let new_location_attr = format!(r#"Location="{}""#, xml_escape(&new_location_value));

    // The old attribute string must appear EXACTLY once — if it appears zero or >1 times, the
    // raw text and the structured model have drifted (e.g. two tracks sharing byte-identical
    // Location, or prior mutation not reflected in raw_xml); fail rather than guess which
    // occurrence to touch.
    let occurrences = xml.raw_xml.matches(&old_location_attr).count();
    if occurrences != 1 {
        log::error!(
            "patch_location: expected exactly 1 occurrence of {old_location_attr:?} in raw_xml, found {occurrences}; refusing to guess"
        );
        return PatchLocationResult::Drifted;
    }
    xml.raw_xml = xml
        .raw_xml
        .replacen(&old_location_attr, &new_location_attr, 1);

    xml.path_index.remove(&path_index_key(&from_norm));
    xml.path_index
        .insert(path_index_key(&normalize_path(to_path)), track_id);
    track.location = new_location_value;
    PatchLocationResult::Patched
}

/// FIX-3: the characters `normalize_path`/`percent_decode` above already know how to reverse,
/// plus the other RFC 3986 reserved characters that show up in real DJ filenames often enough to
/// matter (space above all — virtually every real track name has one). Deliberately does NOT
/// include `/` (path separator, must survive) or `:` (the Windows drive-letter colon, e.g.
/// `C:/Music/...`, must survive unescaped exactly like Rekordbox's own exports).
const LOCATION_PATH_ENCODE_SET: &percent_encoding::AsciiSet = &percent_encoding::CONTROLS
    .add(b' ')
    .add(b'#')
    .add(b'%')
    .add(b'?')
    .add(b'[')
    .add(b']');

/// Percent-encode a plain filesystem path (already `/`-separated) for use as a `Location` value.
/// Without this, a path containing a space or another reserved character (virtually every real
/// track filename) produced a `Location` Rekordbox itself would never emit and can't reliably
/// re-parse on import — this mirrors what real Rekordbox XML exports do (see the module's own
/// `SAMPLE_XML` fixture, whose paths are all already-safe ASCII so the bug was invisible there).
fn encode_location_path(path: &str) -> String {
    percent_encoding::utf8_percent_encode(&path.replace('\\', "/"), LOCATION_PATH_ENCODE_SET)
        .to_string()
}

/// Escape the 5 XML-significant characters in an attribute value the same way Rekordbox itself
/// would emit them — mirrors what `unescape_value` (used at parse time) reverses.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Return the document to write to disk on the merge/export path.
///
/// FIX-2: this used to re-serialize a fresh `DJ_PLAYLISTS` document from ONLY the structured
/// `collection`/`playlists` view — which this module deliberately never fully models (no Rating,
/// BPM, cue points, custom columns, or smart-playlist `Type="4"` nodes) — so every export silently
/// dropped every field/node it didn't understand from the ENTIRE collection, not just the tracks
/// Sift touched. `merge_filed_tracks`/`patch_location` now perform their own targeted text surgery
/// directly on `xml.raw_xml` (new `<TRACK>` rows and playlist `<NODE>` entries inserted next to the
/// existing text, `Entries="N"` counts bumped in place), so `raw_xml` is already the fully up to
/// date document by the time `write` is called — every byte this module doesn't understand,
/// including `Type="4"` smart playlists, survives untouched. `write` now just returns it.
pub fn write(xml: &RekordboxXml) -> String {
    xml.raw_xml.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Vec<u8> {
        SAMPLE_XML.as_bytes().to_vec()
    }

    #[test]
    fn parse_builds_collection_and_index() {
        let xml = fixture();
        let parsed = parse(&xml).unwrap();
        assert_eq!(parsed.collection.len(), 3);
        let t2 = parsed.collection.iter().find(|t| t.track_id == 2).unwrap();
        assert_eq!(t2.artist.as_deref(), Some("Rhythim Is Rhythim"));
        assert_eq!(
            normalize_path(&t2.location),
            PathBuf::from("C:/Music/House/deep/strings.aiff")
        );
    }

    #[test]
    fn parse_builds_path_index_for_lookup() {
        let parsed = parse(&fixture()).unwrap();
        let id = parsed.track_id_for_path(Path::new("C:/Music/Techno/voodoo.flac"));
        assert_eq!(id, Some(3));
        assert_eq!(parsed.track_id_for_path(Path::new("C:/nope.mp3")), None);
    }

    #[test]
    fn parse_builds_playlist_tree_with_shared_track_id() {
        let parsed = parse(&fixture()).unwrap();
        // ROOT → House (2 tracks), Favorites (1 track) — TrackID 2 appears in both.
        assert_eq!(parsed.playlists.len(), 1);
        let PlaylistNode::Folder { name, children } = &parsed.playlists[0] else {
            panic!("ROOT must be a folder");
        };
        assert_eq!(name, "ROOT");
        assert_eq!(children.len(), 2);
        let house = children
            .iter()
            .find(|n| matches!(n, PlaylistNode::Playlist { name, .. } if name == "House"))
            .unwrap();
        let PlaylistNode::Playlist { track_ids, .. } = house else {
            unreachable!()
        };
        assert_eq!(track_ids, &vec![1, 2]);
        let favorites = children
            .iter()
            .find(|n| matches!(n, PlaylistNode::Playlist { name, .. } if name == "Favorites"))
            .unwrap();
        let PlaylistNode::Playlist { track_ids, .. } = favorites else {
            unreachable!()
        };
        assert_eq!(track_ids, &vec![2], "TrackID 2 is shared with House");
    }

    #[test]
    fn parse_rejects_corrupt_xml() {
        let err = parse(b"<not-even-xml").unwrap_err();
        assert!(!err.is_empty());
    }

    fn lib_track(
        path: &str,
        folder: &str,
        artist: &str,
        title: &str,
    ) -> crate::library::LibraryTrack {
        crate::library::LibraryTrack {
            id: 0,
            path: path.to_string(),
            artist: Some(artist.to_string()),
            title: Some(title.to_string()),
            format: None,
            bitrate: None,
            duration: None,
            bpm: None,
            year: None,
            label: None,
            genres: vec![],
            discogs_release_id: None,
            cover_path: None,
            has_cover: false,
            verdict: None,
            folder: Some(folder.to_string()),
        }
    }

    #[test]
    fn merge_adds_missing_tracks_and_creates_folder_playlist() {
        let mut parsed = parse(&fixture()).unwrap();
        let before = parsed.collection.len();
        // "C:/Music/House/deep/strings.aiff" (TrackID 2) already exists — must NOT duplicate.
        // "C:/Music/Disco/new-track.mp3" is new — must be added + filed under a "Disco" playlist.
        let filed = vec![
            lib_track(
                "C:/Music/House/deep/strings.aiff",
                "House/Deep",
                "Rhythim Is Rhythim",
                "Strings of Life",
            ),
            lib_track(
                "C:/Music/Disco/new-track.mp3",
                "Disco",
                "Unknown Artist",
                "New Track",
            ),
        ];
        let added = merge_filed_tracks(&mut parsed, &filed);
        assert_eq!(added, 1, "only the genuinely new track is added");
        assert_eq!(parsed.collection.len(), before + 1);
        let new_track = parsed
            .collection
            .iter()
            .find(|t| normalize_path(&t.location) == Path::new("C:/Music/Disco/new-track.mp3"))
            .unwrap();
        assert_eq!(new_track.artist.as_deref(), Some("Unknown Artist"));

        // A "Disco" playlist now exists under ROOT containing the new track's TrackID.
        let PlaylistNode::Folder { children, .. } = &parsed.playlists[0] else {
            panic!()
        };
        let disco = children
            .iter()
            .find(|n| matches!(n, PlaylistNode::Playlist { name, .. } if name == "Disco"))
            .expect("Disco playlist created");
        let PlaylistNode::Playlist { track_ids, .. } = disco else {
            unreachable!()
        };
        assert_eq!(track_ids, &vec![new_track.track_id]);
    }

    #[test]
    fn merge_never_touches_existing_untouched_playlists() {
        let mut parsed = parse(&fixture()).unwrap();
        let favorites_before = {
            let PlaylistNode::Folder { children, .. } = &parsed.playlists[0] else {
                panic!()
            };
            children
                .iter()
                .find(|n| matches!(n, PlaylistNode::Playlist { name, .. } if name == "Favorites"))
                .cloned()
        };
        // Merge in a track that's already filed under "House" (matches an EXISTING Sift-managed
        // playlist) — "Favorites" (not a Sift folder playlist) must be byte-for-byte untouched.
        let filed = vec![lib_track(
            "C:/Music/House/mr-fingers.mp3",
            "House",
            "Mr Fingers",
            "Can You Feel It",
        )];
        merge_filed_tracks(&mut parsed, &filed);
        let PlaylistNode::Folder { children, .. } = &parsed.playlists[0] else {
            panic!()
        };
        let favorites_after = children
            .iter()
            .find(|n| matches!(n, PlaylistNode::Playlist { name, .. } if name == "Favorites"))
            .cloned();
        assert_eq!(
            favorites_before, favorites_after,
            "non-Sift playlist untouched"
        );
    }

    #[test]
    fn merge_is_idempotent() {
        let mut parsed = parse(&fixture()).unwrap();
        let filed = vec![lib_track("C:/Music/Disco/new-track.mp3", "Disco", "A", "B")];
        let first = merge_filed_tracks(&mut parsed, &filed);
        let second = merge_filed_tracks(&mut parsed, &filed);
        assert_eq!(first, 1);
        assert_eq!(
            second, 0,
            "re-running merge on an already-merged track adds nothing"
        );
    }

    /// FIX-3 regression: a path containing a space (virtually every real track filename) must be
    /// percent-encoded in the written `Location`, matching what Rekordbox itself emits — an
    /// unescaped space in a `file://` URI is not what Rekordbox writes/expects and risks a broken
    /// re-import. `/` and the Windows drive-letter `:` must survive unescaped.
    #[test]
    fn merge_percent_encodes_spaces_in_the_written_location() {
        let mut parsed = parse(&fixture()).unwrap();
        let filed = vec![lib_track(
            "C:/Music/Disco/Diana Ross - Love Hangover.mp3",
            "Disco",
            "Diana Ross",
            "Love Hangover",
        )];
        merge_filed_tracks(&mut parsed, &filed);

        let new_track = parsed
            .collection
            .iter()
            .find(|t| t.artist.as_deref() == Some("Diana Ross"))
            .expect("new track added");
        assert!(
            new_track
                .location
                .contains("Diana%20Ross%20-%20Love%20Hangover.mp3"),
            "spaces percent-encoded in Location, got: {}",
            new_track.location
        );
        assert!(
            new_track
                .location
                .starts_with("file://localhost/C:/Music/Disco/"),
            "drive letter and separators untouched"
        );

        // And the lookup by the plain (unencoded) filesystem path still resolves — the encoding
        // is a write-time concern only, `normalize_path` decodes it back on read.
        assert_eq!(
            parsed.track_id_for_path(Path::new("C:/Music/Disco/Diana Ross - Love Hangover.mp3")),
            Some(new_track.track_id)
        );
    }

    /// FIX-3 regression, `patch_location` side: the SAME percent-encoding must apply when
    /// repairing a Location after a filing/move, not just on initial merge — a track whose new
    /// path contains a space must not end up with a raw, unescaped space in the linked XML.
    #[test]
    fn patch_location_percent_encodes_spaces_in_the_new_location() {
        let mut parsed = parse(&fixture()).unwrap();
        let patched = patch_location(
            &mut parsed,
            "C:/Music/House/deep/strings.aiff",
            "C:/Music/House/Deep Cuts/strings.aiff",
        );
        assert_eq!(patched, PatchLocationResult::Patched);
        let t2 = parsed.collection.iter().find(|t| t.track_id == 2).unwrap();
        assert!(
            t2.location.contains("Deep%20Cuts/strings.aiff"),
            "space percent-encoded, got: {}",
            t2.location
        );
        assert!(
            parsed.raw_xml.contains("Deep%20Cuts/strings.aiff"),
            "raw_xml carries the encoded form"
        );
    }

    #[test]
    fn patch_location_updates_only_that_tracks_location_byte_identical_elsewhere() {
        let mut parsed = parse(&fixture()).unwrap();
        let original_raw = parsed.raw_xml.clone();

        let patched = patch_location(
            &mut parsed,
            "C:/Music/House/deep/strings.aiff",
            "C:/Music/House/Deep/strings.aiff", // recased/moved
        );
        assert_eq!(patched, PatchLocationResult::Patched);

        // Structured view updated.
        let t2 = parsed.collection.iter().find(|t| t.track_id == 2).unwrap();
        assert_eq!(
            normalize_path(&t2.location),
            PathBuf::from("C:/Music/House/Deep/strings.aiff")
        );
        assert_eq!(
            parsed.track_id_for_path(Path::new("C:/Music/House/Deep/strings.aiff")),
            Some(2)
        );
        // FIX-6: the path_index lookup key is case-folded, and "deep" vs "Deep" is the ONLY
        // difference between the old and new path here — so the pre-patch (lowercase) path still
        // resolves to the same track post-patch, same as it would pre-fix for a genuinely
        // unrelated recase-only rename with no other change. This is the fix, not a leftover stale
        // pointer: see `patch_location_old_path_stops_resolving_after_a_real_move` below for the
        // case that must still return None (moving to an actually different path).
        assert_eq!(
            parsed.track_id_for_path(Path::new("C:/Music/House/deep/strings.aiff")),
            Some(2)
        );

        // raw_xml: EXACTLY one substring differs (the Location value) — verify by diffing line
        // by line, every other line must be byte-identical, and the TrackID="2" line must still
        // contain every other original attribute untouched (Name, Artist).
        let before_lines: Vec<&str> = original_raw.lines().collect();
        let after_lines: Vec<&str> = parsed.raw_xml.lines().collect();
        assert_eq!(
            before_lines.len(),
            after_lines.len(),
            "no lines added or removed"
        );
        let mut changed_lines = 0;
        for (b, a) in before_lines.iter().zip(after_lines.iter()) {
            if b != a {
                changed_lines += 1;
                assert!(
                    a.contains(r#"TrackID="2""#),
                    "the only changed line is TrackID 2's row"
                );
                assert!(
                    a.contains(r#"Name="Strings of Life""#),
                    "Name attribute untouched"
                );
                assert!(
                    a.contains(r#"Artist="Rhythim Is Rhythim""#),
                    "Artist attribute untouched"
                );
                assert!(a.contains("Deep/strings.aiff"), "new Location present");
            }
        }
        assert_eq!(
            changed_lines, 1,
            "exactly one line changed — the patched TRACK row"
        );
    }

    #[test]
    fn patch_location_returns_not_tracked_when_path_unknown() {
        let mut parsed = parse(&fixture()).unwrap();
        let patched = patch_location(&mut parsed, "C:/not/tracked.mp3", "C:/elsewhere.mp3");
        assert_eq!(patched, PatchLocationResult::NotTracked);
    }

    /// FIX-7 regression: when the SAME Location text appears twice in raw_xml (two collection
    /// tracks sharing a byte-identical Location — a drifted/corrupt file), patch_location must
    /// report `Drifted`, distinct from the ordinary `NotTracked` no-op, so a caller can surface
    /// genuine drift to the user instead of it only ever reaching a server log.
    #[test]
    fn patch_location_reports_drifted_on_ambiguous_raw_xml_match() {
        let dup_location_xml = r#"<?xml version="1.0" encoding="UTF-8"?>

<DJ_PLAYLISTS Version="1.0.0">
  <PRODUCT Name="rekordbox" Version="6.7.7" Company="Pioneer DJ"/>
  <COLLECTION Entries="2">
    <TRACK TrackID="1" Name="A" Artist="X" Location="file://localhost/C:/Music/dup.mp3"/>
    <TRACK TrackID="2" Name="B" Artist="Y" Location="file://localhost/C:/Music/dup.mp3"/>
  </COLLECTION>
  <PLAYLISTS>
    <NODE Type="0" Name="ROOT" Count="0"/>
  </PLAYLISTS>
</DJ_PLAYLISTS>
"#;
        let mut parsed = parse(dup_location_xml.as_bytes()).unwrap();
        let original_raw = parsed.raw_xml.clone();

        let result = patch_location(&mut parsed, "C:/Music/dup.mp3", "C:/Music/moved.mp3");
        assert_eq!(result, PatchLocationResult::Drifted);
        assert_eq!(
            parsed.raw_xml, original_raw,
            "nothing changed on a Drifted result"
        );
    }

    /// FIX-6 regression: `track_id_for_path` must not miss a lookup that differs from the XML's
    /// stored `Location` only by drive-letter/segment casing — a plain `PathBuf` equality is
    /// case-sensitive, but Windows/macOS (Sift's only targets) are case-insensitive-preserving
    /// filesystems, so a caller (Sift's own scanner, a user-typed path) can legitimately pass a
    /// differently-cased-but-identical path and must still get a hit.
    #[test]
    fn track_id_for_path_matches_across_drive_letter_and_segment_casing() {
        let parsed = parse(&fixture()).unwrap();
        // Fixture Location is "file://localhost/C:/Music/House/mr-fingers.mp3" (TrackID 1).
        assert_eq!(
            parsed.track_id_for_path(Path::new("c:/music/house/mr-fingers.mp3")),
            Some(1)
        );
        assert_eq!(
            parsed.track_id_for_path(Path::new("C:/MUSIC/HOUSE/MR-FINGERS.MP3")),
            Some(1)
        );
    }

    /// Counterpart to the case-insensitivity fix above: moving to a genuinely DIFFERENT path (not
    /// just a recase) must still make the OLD path stop resolving — the fix folds case, it does
    /// not make every old path a permanent alias forever.
    #[test]
    fn patch_location_old_path_stops_resolving_after_a_real_move() {
        let mut parsed = parse(&fixture()).unwrap();
        let patched = patch_location(
            &mut parsed,
            "C:/Music/House/deep/strings.aiff",
            "C:/Music/Techno/deep/strings.aiff", // genuinely different folder, not just recased
        );
        assert_eq!(patched, PatchLocationResult::Patched);
        assert_eq!(
            parsed.track_id_for_path(Path::new("C:/Music/Techno/deep/strings.aiff")),
            Some(2)
        );
        assert_eq!(
            parsed.track_id_for_path(Path::new("C:/Music/House/deep/strings.aiff")),
            None,
            "the old (now-vacated) path must no longer resolve"
        );
    }

    #[test]
    fn write_then_reparse_round_trips_collection_and_playlists() {
        let mut parsed = parse(&fixture()).unwrap();
        let filed = vec![lib_track("C:/Music/Disco/new-track.mp3", "Disco", "A", "B")];
        merge_filed_tracks(&mut parsed, &filed);

        let xml_text = write(&parsed);
        let reparsed = parse(xml_text.as_bytes()).unwrap();

        assert_eq!(reparsed.collection.len(), parsed.collection.len());
        assert!(reparsed
            .track_id_for_path(Path::new("C:/Music/Disco/new-track.mp3"))
            .is_some());
        // Playlist tree shape preserved (ROOT → House/Favorites/Disco).
        let PlaylistNode::Folder { children, .. } = &reparsed.playlists[0] else {
            panic!()
        };
        let names: Vec<&str> = children
            .iter()
            .map(|n| match n {
                PlaylistNode::Folder { name, .. } | PlaylistNode::Playlist { name, .. } => {
                    name.as_str()
                }
            })
            .collect();
        assert!(names.contains(&"House"));
        assert!(names.contains(&"Favorites"));
        assert!(names.contains(&"Disco"));
    }

    /// A fixture carrying fields this module deliberately never models: a `Rating` attribute on
    /// an existing TRACK, and a smart playlist (`Type="4"`, with a `<PRODUCT/>`-style filter
    /// child it doesn't understand either) alongside the plain ROOT folder tree. Used to prove
    /// FIX-2: none of this may be dropped by a merge+write cycle.
    fn fixture_with_unmodeled_fields() -> Vec<u8> {
        r#"<?xml version="1.0" encoding="UTF-8"?>

<DJ_PLAYLISTS Version="1.0.0">
  <PRODUCT Name="rekordbox" Version="6.7.7" Company="Pioneer DJ"/>
  <COLLECTION Entries="2">
    <TRACK TrackID="1" Name="Can You Feel It" Artist="Mr Fingers" Rating="255" BPM="123.00" Location="file://localhost/C:/Music/House/mr-fingers.mp3"/>
    <TRACK TrackID="2" Name="Strings of Life" Artist="Rhythim Is Rhythim" Location="file://localhost/C:/Music/House/deep/strings.aiff"/>
  </COLLECTION>
  <PLAYLISTS>
    <NODE Type="0" Name="ROOT" Count="2">
      <NODE Type="1" Name="House" KeyType="0" Entries="1">
        <TRACK Key="1"/>
      </NODE>
      <NODE Type="4" Name="Smart 120+ BPM" Uri="something-rekordbox-only-understands"/>
    </NODE>
  </PLAYLISTS>
</DJ_PLAYLISTS>
"#
        .as_bytes()
        .to_vec()
    }

    /// FIX-2 regression: the judge of the whole fix. Merging a genuinely new filed track in and
    /// writing the result must NOT drop the `Rating`/`BPM` attributes on an EXISTING track, nor
    /// the `Type="4"` smart playlist node — both survive a merge+write cycle byte-for-byte,
    /// because `write()` now returns the surgically-patched `raw_xml` instead of re-serializing
    /// from the structured (necessarily lossy) `collection`/`playlists` view.
    #[test]
    fn merge_and_write_preserves_unmodeled_rating_bpm_and_smart_playlist() {
        let mut parsed = parse(&fixture_with_unmodeled_fields()).unwrap();
        let filed = vec![lib_track("C:/Music/Disco/new-track.mp3", "Disco", "A", "B")];
        merge_filed_tracks(&mut parsed, &filed);

        let out = write(&parsed);

        assert!(
            out.contains(r#"Rating="255""#),
            "Rating on the untouched existing track survives"
        );
        assert!(
            out.contains(r#"BPM="123.00""#),
            "BPM on the untouched existing track survives"
        );
        assert!(
            out.contains(r#"<NODE Type="4" Name="Smart 120+ BPM" Uri="something-rekordbox-only-understands"/>"#),
            "the Type=\"4\" smart playlist node survives byte-for-byte, unparsed"
        );

        // And the new track/playlist ARE genuinely present (the fix isn't a no-op).
        assert!(
            out.contains("new-track.mp3")
                || out.contains("new%2Dtrack.mp3")
                || out.contains("new-track")
        );
        assert!(out.contains(r#"Name="Disco""#));

        // The result still re-parses cleanly with everything the module DOES model intact.
        let reparsed = parse(out.as_bytes()).unwrap();
        assert_eq!(reparsed.collection.len(), 3);
        assert!(reparsed
            .track_id_for_path(Path::new("C:/Music/Disco/new-track.mp3"))
            .is_some());
    }

    /// FIX-2 regression, narrower: writing with NO merge at all (nothing new to add) must return
    /// `raw_xml` completely unchanged — proves `write()` is a pure passthrough now, not a
    /// from-scratch rebuild that happens to look similar.
    #[test]
    fn write_with_no_new_tracks_returns_raw_xml_unchanged() {
        let parsed = parse(&fixture_with_unmodeled_fields()).unwrap();
        let original = parsed.raw_xml.clone();
        let out = write(&parsed);
        assert_eq!(
            out, original,
            "write() must be a byte-identical passthrough when nothing changed"
        );
    }

    /// FIX-2 regression: filing a new track into a folder whose playlist ALREADY EXISTS (not a
    /// brand-new one) must append the `<TRACK Key>` inside that existing NODE and bump its
    /// `Entries` count, while leaving the smart playlist sibling completely untouched.
    #[test]
    fn merge_appends_into_existing_playlist_node_and_bumps_entries() {
        let mut parsed = parse(&fixture_with_unmodeled_fields()).unwrap();
        // "House" playlist already exists with TrackID 1 only; file a genuinely NEW track
        // (different path than any existing collection entry) into that same folder.
        let filed = vec![lib_track(
            "C:/Music/House/another.mp3",
            "House",
            "Someone",
            "Another Track",
        )];
        merge_filed_tracks(&mut parsed, &filed);

        let out = write(&parsed);
        assert!(
            out.contains(r#"Name="House" KeyType="0" Entries="2""#),
            "Entries bumped 1 -> 2"
        );
        // TrackID 1 (pre-existing) and TrackID 3 (newly-added — max(1,2)+1) both present as keys.
        assert!(out.contains(r#"<TRACK Key="1"/>"#) && out.contains(r#"<TRACK Key="3"/>"#));
        assert!(
            out.contains(r#"<NODE Type="4" Name="Smart 120+ BPM" Uri="something-rekordbox-only-understands"/>"#),
            "sibling smart playlist untouched by the House-node edit"
        );
    }
}
