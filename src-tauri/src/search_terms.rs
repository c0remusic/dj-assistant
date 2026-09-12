//! Construction des termes de recherche Discogs à partir d'un nom de fichier sale.
//!
//! # Pourquoi ce module existe séparément de `naming.rs`
//!
//! `naming::Canonical` porte l'identité qu'on ÉCRIT : nom de fichier de sortie (`render_filename`)
//! et tags embarqués (`tag_title`). Elle doit rester prudente — mieux vaut un champ vide qu'un
//! mauvais nom écrit sur le disque. C'est pour ça que `naming::is_clean` est un portail de REJET :
//! un `[` dans le nom et le nom entier est écarté.
//!
//! Une requête réseau n'a pas ces contraintes. Elle est jetable : si elle se trompe, on ne perd
//! qu'un aller-retour HTTP. Elle peut donc être agressive là où `Canonical` doit être timide.
//!
//! Faire porter les deux contrats au même type coûtait cher, mesuré le 2026-07-28 sur 2 714 pistes
//! réelles : ~54 % de la bibliothèque partait avec une requête structurellement cassée (artiste
//! vide ou artiste = numéro de piste). Voir
//! `docs/superpowers/changes/2026-07-28-discogs-dirty-names/design.md` §1.
//!
//! # Contrat
//!
//! `build` est PURE : pas d'I/O, pas de verrou, pas d'horloge. Entrées texte → sortie texte. C'est
//! ce qui permet de la brancher sur les 77 cas réels de `search_corpus.rs` et d'avoir un chiffre
//! plutôt qu'une impression.
//!
//! Rien de ce que produit ce module ne doit atteindre le disque. Si un jour un appelant veut s'en
//! servir pour nommer un fichier, il faut repasser par `Canonical` et sa confiance.

/// Un essai de recherche, du plus spécifique au plus dégradé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attempt {
    /// La chaîne à envoyer en `q=`.
    pub q: String,
    /// Étiquette stable pour le log et les tests — jamais affichée à l'utilisateur.
    pub label: &'static str,
}

/// Ce qu'on a réussi à extraire, plus la cascade d'essais à tenter dans l'ordre.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Terms {
    /// Vide quand aucune source (nom, dossier) ne porte d'artiste — c'est un résultat légitime,
    /// pas un échec : la cascade sait chercher sur le titre seul.
    pub artist: String,
    pub title: String,
    pub version: Option<String>,
    /// Du plus spécifique au plus dégradé. Jamais vide si `artist` ou `title` l'est pas.
    pub ladder: Vec<Attempt>,
}

// ---------------------------------------------------------------------------------------------
// Vocabulaire
// ---------------------------------------------------------------------------------------------

/// Mots qui marquent une parenthèse/un crochet comme étant une VERSION plutôt que du bruit.
/// Utilisé en test de sous-chaîne sur le contenu mis en minuscules, jamais sur le nom entier.
const VERSION_WORDS: &[&str] = &[
    "mix",
    "remix",
    "rmx",
    "dub",
    "edit",
    "version",
    "instrumental",
    "original",
    "extended",
    "radio",
    "club",
    "vocal",
    "rework",
    "reprise",
    "bootleg",
    "vip",
    "acapella",
    "accapella",
    "live",
    "remaster",
];

/// Marqueurs de qualité/format. Contrairement à `naming::DROP` qui compare par mot EXACT (et rate
/// donc `-320kbps-`, `[320]`, `320k`), ceux-ci sont cherchés en sous-chaîne dans un groupe
/// parenthésé, et par motif ailleurs.
const QUALITY_WORDS: &[&str] = &[
    "kbps",
    "khz",
    " hz",
    "flac",
    "wav",
    "aiff",
    "mp3",
    "vbr",
    "cbr",
    "lossless",
    "vinyl rip",
    "web-dl",
    "cdrip",
    "cd rip",
    "24bits",
    "16bit",
    "24bit",
];

/// Mots qui trahissent un nom de label en fin de chaîne (`… – Distants Records`).
const LABEL_WORDS: &[&str] = &[
    "records",
    "recordings",
    "records.",
    "rec.",
    "muzik",
    "musique",
];

// ---------------------------------------------------------------------------------------------
// Point d'entrée
// ---------------------------------------------------------------------------------------------

/// Construit les termes de recherche depuis le nom de fichier sans extension et le nom du dossier
/// parent SEUL (jamais un chemin complet — on ne veut pas miner `C:\Users\…`).
///
/// Le dossier n'est consulté que pour combler un artiste manquant : sur la bibliothèque mesurée,
/// 243 pistes n'ont d'artiste QUE là (`(SOMA 21) Slam-Snapshots` → `A1-Stepback`). Il n'écrase
/// jamais un artiste trouvé dans le nom.
pub fn build(stem: &str, folder: &str) -> Terms {
    let (mut artist, title, version) = extract_from(stem);

    // Le dossier ne comble qu'un trou, il n'arbitre jamais. Un dossier comme `2_040924` ou
    // `complete` ne produit pas de décomposition plausible et est ignoré en silence — sans quoi
    // il injecterait le même faux artiste dans des centaines de requêtes d'un coup.
    if artist.is_empty() {
        if let Some(folder_artist) = mine_folder_artist(folder) {
            artist = folder_artist;
        }
    }

    // Un titre vide avec un artiste plein n'a pas de sens comme requête : on préfère tout vider
    // plutôt qu'envoyer un nom d'artiste seul, qui ramène sa discographie entière.
    if title.is_empty() {
        artist.clear();
    }

    let ladder = build_ladder(&artist, &title, version.as_deref(), folder, stem);
    Terms {
        artist,
        title,
        version,
        ladder,
    }
}

/// Tous les mots du nom dégraissé, séparateurs compris, réduits à des espaces.
///
/// C'est le dernier recours de la cascade, et il vaut par ce qu'il ne suppose PAS : on n'a pas
/// besoin de savoir où finit l'artiste pour envoyer les bons mots. Sur
/// `2-Gunne-What-I-Like--Fi-LOPZUP`, le découpage en champs échoue mais `gunne what i like` est
/// exactement ce qu'un humain taperait — et c'est ce que Discogs indexe.
fn all_words(stem: &str) -> String {
    let (cleaned, _) = degrease(stem);
    let spaced: String = cleaned
        .chars()
        .map(|c| if c == '-' { ' ' } else { c })
        .collect();
    collapse_ws(&spaced)
}

// ---------------------------------------------------------------------------------------------
// Étage 1 — normalisation
// ---------------------------------------------------------------------------------------------

/// Uniformise ce qui peut l'être SANS perdre d'information de structure.
///
/// Le tiret demi-cadratin (U+2013) et le cadratin (U+2014) servent de séparateur de champ dans les
/// noms observés (`CJ Art – Acedia`) et deviennent donc un `-` ASCII. Le tiret U+2010, lui, est
/// laissé tel quel : il apparaît À L'INTÉRIEUR d'un nom d'artiste (`Markus Enochson feat. E‐Man`)
/// et le convertir en fabriquerait un faux séparateur.
fn normalise(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\u{2013}' | '\u{2014}' => out.push('-'),
            '\u{00A0}' | '\u{2007}' | '\u{202F}' => out.push(' '),
            c => out.push(c),
        }
    }
    // `_-_` est un séparateur de CHAMP là où `_` isolé est un séparateur de MOT : le traiter
    // d'abord évite que la conversion en espaces ne les confonde.
    out = out.replace("_-_", " - ");
    out = out.replace('_', " ");
    collapse_ws(&out)
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------------------------------------
// Étage 2 — retrait du bruit
// ---------------------------------------------------------------------------------------------

/// Vrai si le contenu d'un groupe (parenthèse/crochet) désigne une version.
fn is_version_content(inner: &str) -> bool {
    let low = inner.to_lowercase();
    if QUALITY_WORDS.iter().any(|q| low.contains(q)) {
        return false;
    }
    VERSION_WORDS.iter().any(|w| {
        // Frontière de mot à gauche pour ne pas prendre "prehistoric" pour "historic"… et surtout
        // pour que "remix" n'active pas "mix" deux fois (sans conséquence ici, mais l'intention
        // est de tester des mots, pas des fragments).
        low.split(|c: char| !c.is_alphanumeric()).any(|t| t == *w)
    })
}

/// Vrai si le contenu ressemble à une référence de catalogue (`FAR11 - 2005`, `BU 002`, `spm007`,
/// `DIS009`) : au moins un chiffre, pas de mot de version, et court.
fn is_catalog_content(inner: &str) -> bool {
    let t = inner.trim();
    if t.is_empty() || t.len() > 24 {
        return false;
    }
    t.chars().any(|c| c.is_ascii_digit()) && !is_version_content(t)
}

/// Vrai si le contenu n'est que du marqueur de qualité (`320 kbps`, `FLAC`, `mp3`).
fn is_quality_content(inner: &str) -> bool {
    let low = inner.to_lowercase();
    if QUALITY_WORDS.iter().any(|q| low.contains(q)) {
        return true;
    }
    // `(320)`, `[128]` seuls.
    let t = low.trim();
    matches!(t, "320" | "256" | "192" | "128" | "v0" | "v2" | "320k")
}

fn is_url_content(inner: &str) -> bool {
    let low = inner.to_lowercase();
    low.contains("www.") || low.contains("http") || low.contains(".com") || low.contains(".org")
}

/// Une paire de délimiteurs trouvée dans la chaîne. `close` est l'index de l'octet du délimiteur
/// fermant, `open` celui de l'ouvrant.
struct Group {
    open: usize,
    close: usize,
    inner: String,
}

/// Trouve le premier groupe délimité à partir de `from`. Accepte les paires DÉPAREILLÉES
/// (`(spm007]`) : elles existent dans la vraie vie et un appariement strict laisse passer tout le
/// groupe dans la requête.
fn next_group(s: &str, from: usize) -> Option<Group> {
    let bytes: Vec<char> = s.chars().collect();
    let idx_of =
        |ci: usize| -> usize { s.char_indices().nth(ci).map(|(i, _)| i).unwrap_or(s.len()) };
    let mut i = from;
    while i < bytes.len() {
        if bytes[i] == '(' || bytes[i] == '[' || bytes[i] == '{' {
            let mut j = i + 1;
            while j < bytes.len() {
                if bytes[j] == ')' || bytes[j] == ']' || bytes[j] == '}' {
                    let inner: String = bytes[i + 1..j].iter().collect();
                    return Some(Group {
                        open: idx_of(i),
                        close: idx_of(j) + bytes[j].len_utf8(),
                        inner,
                    });
                }
                // Un nouveau groupe ouvrant avant toute fermeture : le premier ouvrant était
                // orphelin, on repart de celui-ci.
                if bytes[j] == '(' || bytes[j] == '[' || bytes[j] == '{' {
                    break;
                }
                j += 1;
            }
            i = if j < bytes.len() && (bytes[j] == '(' || bytes[j] == '[' || bytes[j] == '{') {
                j
            } else {
                i + 1
            };
        } else {
            i += 1;
        }
    }
    None
}

/// Retire tous les groupes de bruit et récupère au passage la DERNIÈRE version rencontrée.
///
/// Position significative : un groupe en TÊTE qui ressemble à un catalogue est un numéro de
/// référence (`[BU 002] DJ Gregory - Freeze`) ; le même contenu en QUEUE serait une version.
fn strip_groups(s: &str) -> (String, Option<String>) {
    let mut out = s.to_string();
    let mut version: Option<String> = None;
    let mut guard = 0;
    loop {
        guard += 1;
        if guard > 32 {
            break; // filet : aucune chaîne réelle n'a 32 groupes, une boucle infinie serait pire
        }
        let Some(g) = next_group(&out, 0) else { break };
        let leading = out[..g.open].trim().is_empty();
        let trailing = out[g.close..].trim().is_empty();
        let disqualified = is_quality_content(&g.inner)
            || is_url_content(&g.inner)
            || is_catalog_content(&g.inner);
        // Un groupe en QUEUE qui n'est ni de la qualité, ni une URL, ni un catalogue EST la
        // version, même sans mot-clé reconnu : `[benonedit]`, `[original]` n'ont pas de forme
        // canonique et exiger un mot du vocabulaire les perdrait. En tête ou au milieu, la même
        // chaîne serait une référence de release, d'où le test de position.
        let keep_as_version =
            !leading && !disqualified && (trailing || is_version_content(&g.inner));
        if keep_as_version {
            version = Some(collapse_ws(&g.inner));
        }
        let mut next = String::with_capacity(out.len());
        next.push_str(&out[..g.open]);
        next.push(' ');
        next.push_str(&out[g.close..]);
        out = collapse_ws(&next);
    }
    (out, version)
}

/// Retire les jetons d'URL et de qualité qui ne sont dans aucun groupe
/// (`…Original Mix-www.groovytunes.org`, `-320kbps-`).
fn strip_loose_noise(s: &str) -> String {
    let mut kept: Vec<String> = Vec::new();
    for word in s.split_whitespace() {
        // Une URL peut être collée à du texte utile par un tiret : on coupe au point d'accroche.
        let w = match find_url_start(word) {
            Some(0) => continue,
            Some(k) => word[..k].trim_end_matches(['-', '.', ',']).to_string(),
            None => word.to_string(),
        };
        if w.is_empty() {
            continue;
        }
        let low = w.to_lowercase();
        let bare = low.trim_matches(|c: char| !c.is_alphanumeric());
        if matches!(
            bare,
            "kbps" | "khz" | "flac" | "wav" | "aiff" | "mp3" | "cbr" | "vbr" | "320kbps"
        ) {
            continue;
        }
        if matches!(bare, "320" | "256" | "192" | "128") {
            continue;
        }
        kept.push(w);
    }
    collapse_ws(&kept.join(" "))
}

/// Index du début d'une URL dans un mot, s'il y en a une.
fn find_url_start(word: &str) -> Option<usize> {
    let low = word.to_lowercase();
    ["www.", "http://", "https://"]
        .iter()
        .filter_map(|p| low.find(p))
        .min()
}

/// Retire un suffixe de groupe de release (`-ccat`, `-idc`, `-LOPZUP`), un hash (`-7d468690`) ou un
/// numéro d'ordre (`-001`) en fin de chaîne.
///
/// Trois garde-fous, chacun payé par un cas réel du corpus :
///
/// 1. Ne coupe QUE s'il reste de la matière devant — sans quoi le titre `-ism` serait entièrement
///    mangé, il ressemble trait pour trait à un tag de groupe.
/// 2. Un tag en minuscules n'est retiré que si le nom est de forme « scène » (`scene_shaped`,
///    c'est-à-dire qu'il contient des soulignés). Sinon `sci-fi` perdrait son `-fi`.
/// 3. Longueur bornée à 5 en minuscules — sans quoi `…cano-delirium` perdrait `delirium`, qui est
///    le titre. Les tags réels observés font 2 à 4 caractères (`sq`, `dh`, `idc`, `ccat`, `scmt`).
///
/// Les majuscules (`LOPZUP`) et les formes numériques (`-001`, hash 8 chiffres hexadécimaux) sont
/// sans ambiguïté et se retirent toujours.
fn strip_scene_suffix(s: &str, scene_shaped: bool) -> String {
    let mut cur = s.trim().to_string();
    for _ in 0..3 {
        let Some(pos) = cur.rfind('-') else { break };
        let (head, tail) = cur.split_at(pos);
        let tok = tail[1..].trim();
        let head_trim = head.trim_end_matches('-').trim();
        if head_trim.chars().filter(|c| c.is_alphanumeric()).count() < 4 {
            break;
        }
        let alnum = !tok.is_empty() && tok.chars().all(|c| c.is_ascii_alphanumeric());
        let is_hash = alnum && tok.len() == 8 && tok.chars().all(|c| c.is_ascii_hexdigit());
        let is_ordinal = alnum && tok.len() <= 3 && tok.chars().all(|c| c.is_ascii_digit());
        let is_upper_tag =
            alnum && (3..=8).contains(&tok.len()) && tok.chars().all(|c| c.is_ascii_uppercase());
        let is_lower_tag = alnum
            && scene_shaped
            && (2..=5).contains(&tok.len())
            && tok.chars().all(|c| c.is_ascii_lowercase());
        if is_hash || is_ordinal || is_upper_tag || is_lower_tag {
            cur = head_trim.to_string();
        } else {
            break;
        }
    }
    cur
}

/// Retire un marqueur de position en tête : numéro de piste (`01 `, `01-14 `, `132.01--`,
/// `2-07. `) ou code de face vinyle (`A1. `, `(a1) `, `A1-`, `B2 `).
///
/// N'agit qu'UNE fois pour chaque famille : `01 89 (Original Mix)` doit garder `89` comme titre.
fn strip_leading_position(s: &str) -> String {
    let cur = s.trim();
    if let Some(rest) = strip_leading_side(cur) {
        return rest.to_string();
    }
    if let Some(rest) = strip_leading_track_no(cur) {
        // Un numéro de disque-piste peut être suivi d'un code de face (`2-07. Alaska`) — non, mais
        // il peut être suivi d'un séparateur résiduel, déjà consommé par strip_leading_track_no.
        return rest.to_string();
    }
    cur.to_string()
}

/// `A1. `, `(a1) `, `A1-`, `B2 ` en tête.
///
/// `require_digit` protège l'ARTISTE : `A. Jas` est un vrai nom, et sans cette exigence il serait
/// décapité en `Jas`. Côté TITRE en revanche, `Magnetic Disorder -  A. The Observer` porte une
/// face nue qu'il faut retirer — l'appelant lève donc l'exigence pour ce champ-là seulement.
fn strip_leading_side_opt(s: &str, require_digit: bool) -> Option<&str> {
    let b: Vec<char> = s.chars().collect();
    let mut i = 0;
    let parenthesised = b.first() == Some(&'(');
    if parenthesised {
        i += 1;
    }
    let letter = *b.get(i)?;
    if !letter.is_ascii_alphabetic() || !('a'..='h').contains(&letter.to_ascii_lowercase()) {
        return None;
    }
    i += 1;
    let digits_start = i;
    while i < b.len() && b[i].is_ascii_digit() && i - digits_start < 2 {
        i += 1;
    }
    if require_digit && i == digits_start {
        return None; // pas de chiffre → ce n'est pas une face
    }
    if parenthesised {
        if b.get(i) != Some(&')') {
            return None;
        }
        i += 1;
    }
    // Il faut un délimiteur derrière, sinon `A1` pourrait être le début d'un vrai mot.
    let mut saw_delim = false;
    while i < b.len() && matches!(b[i], ' ' | '.' | '-' | ')' | ':') {
        saw_delim = true;
        i += 1;
    }
    if !saw_delim {
        return None;
    }
    let byte_idx = s.char_indices().nth(i).map(|(k, _)| k).unwrap_or(s.len());
    let rest = s[byte_idx..].trim();
    if rest.is_empty() {
        return Some("");
    }
    Some(rest)
}

/// Face vinyle en tête d'un nom complet : exige le chiffre (voir `strip_leading_side_opt`).
fn strip_leading_side(s: &str) -> Option<&str> {
    strip_leading_side_opt(s, true)
}

/// `01 `, `01-14 `, `132.01--`, `2-07. `, `01. `. Refuse une année (`2003 - Force Feeling`) —
/// 4 chiffres n'est jamais un numéro de piste, et l'année est un champ à part entière.
fn strip_leading_track_no(s: &str) -> Option<&str> {
    let b: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    if i == 0 || i > 3 {
        return None;
    }
    // Second groupe de chiffres (disque-piste) séparé par `.` ou `-`.
    if i < b.len() && matches!(b[i], '.' | '-') {
        let mut j = i + 1;
        let start = j;
        while j < b.len() && b[j].is_ascii_digit() && j - start < 3 {
            j += 1;
        }
        if j > start {
            i = j;
        }
    }
    // Le tiret n'appartient au numéro que dans trois situations, chacune tirée d'un cas réel :
    //   - collé aux chiffres            `02-maetrik`      → séparateur
    //   - suivi d'une espace ou d'un `-` `01 - abduction`  → séparateur
    //   - dans la continuité d'un tiret déjà consommé `132.01--A. Jas`
    // Hors de là il fait partie de ce qui suit : dans `10. -ism`, l'avaler donnerait `ism`.
    let mut saw_delim = false;
    let mut dash_run = false;
    let digits_end = i;
    while i < b.len() {
        match b[i] {
            ' ' | '.' | ')' | '_' => {
                saw_delim = true;
                i += 1;
            }
            '-' if i == digits_end
                || dash_run
                || matches!(b.get(i + 1), None | Some(' ') | Some('-')) =>
            {
                saw_delim = true;
                dash_run = true;
                i += 1;
            }
            _ => break,
        }
    }
    if !saw_delim {
        return None; // `100Hz`, `89bpm` : le chiffre fait partie du mot
    }
    let byte_idx = s.char_indices().nth(i).map(|(k, _)| k).unwrap_or(s.len());
    let rest = s[byte_idx..].trim();
    if rest.is_empty() {
        return None;
    }
    Some(rest)
}

// ---------------------------------------------------------------------------------------------
// Étage 3 — découpage en champs
// ---------------------------------------------------------------------------------------------

/// Découpe en champs sur le séparateur le plus fiable présent.
///
/// Ordre délibéré : ` - ` d'abord parce qu'il est explicite et ne peut pas être un tiret intramot ;
/// le tiret collé en DERNIER parce qu'il coupe `sci-fi`, `U-Too`, `QA 0-127`. Un nom qui contient
/// ` - ` n'utilise donc jamais la règle risquée.
fn split_fields(s: &str) -> Vec<String> {
    for sep in [" - ", " -", "- ", "--", ".-"] {
        if s.contains(sep) {
            let parts: Vec<String> = s
                .split(sep)
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect();
            if parts.len() >= 2 {
                return parts;
            }
        }
    }
    if s.contains('-') {
        let parts: Vec<String> = s
            .split('-')
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .collect();
        if parts.len() >= 2 {
            return parts;
        }
    }
    vec![s.trim().to_string()]
}

/// Vrai si le champ n'est qu'un marqueur de position (`A2`, `B`, `8`, `01`) — un repère de pressage
/// ou de tracklist, jamais un artiste ni un titre.
fn is_positional_field(f: &str) -> bool {
    let t = f.trim().trim_end_matches('.');
    if t.is_empty() || t.len() > 3 {
        return false;
    }
    if t.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }
    let mut it = t.chars();
    let Some(first) = it.next() else { return false };
    first.is_ascii_alphabetic()
        && ('a'..='h').contains(&first.to_ascii_lowercase())
        && it.clone().count() <= 2
        && it.all(|c| c.is_ascii_digit())
}

/// Vrai si le champ est une année seule.
fn is_year_field(f: &str) -> bool {
    let t = f.trim();
    t.len() == 4
        && t.chars().all(|c| c.is_ascii_digit())
        && (t.starts_with("19") || t.starts_with("20"))
}

fn is_label_field(f: &str) -> bool {
    let low = f.to_lowercase();
    LABEL_WORDS.iter().any(|w| low.contains(w))
}

/// Clé de comparaison laxiste pour repérer un champ répété (l'artiste redit en fin de nom dans les
/// compilations).
fn loose_key(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Étage 4 — attribution des rôles
// ---------------------------------------------------------------------------------------------

/// Sépare une version accolée en fin de titre sans parenthèses
/// (`Pixel Waterfall Original Mix`, `Fluent Remix`).
fn split_trailing_version_phrase(title: &str) -> (String, Option<String>) {
    let words: Vec<&str> = title.split_whitespace().collect();
    if words.len() < 2 {
        return (title.to_string(), None);
    }
    let last = words[words.len() - 1].to_lowercase();
    let last_bare: String = last.chars().filter(|c| c.is_alphanumeric()).collect();
    if !matches!(
        last_bare.as_str(),
        "mix" | "remix" | "rmx" | "edit" | "version" | "dub"
    ) {
        return (title.to_string(), None);
    }
    // Le mot juste avant qualifie la version (`Original Mix`, `Club Version`, `Fluent Remix`).
    // On en prend un seul : au-delà, on mangerait du titre.
    let start = words.len() - 2;
    if start == 0 {
        return (title.to_string(), None); // tout le titre serait la version
    }
    let head = words[..start].join(" ");
    let ver = words[start..].join(" ");
    (head, Some(ver))
}

/// Un titre réduit à un mot générique ne dit rien : mieux vaut le vide, qui déclenche la bonne
/// cascade, qu'une requête sur « Untitled » qui ramènera n'importe quoi.
fn is_generic_title(t: &str) -> bool {
    let low: String = t
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect();
    matches!(
        low.as_str(),
        "untitled" | "unititled" | "track" | "audio" | ""
    )
}

/// Cœur de l'extraction : nom de fichier sale → (artiste, titre, version).
/// Retire tout le bruit sans encore décider qui est l'artiste : c'est l'étape commune à
/// l'extraction des champs et à l'essai « tous les mots » de la cascade.
fn degrease(stem: &str) -> (String, Option<String>) {
    // Décidé sur le nom BRUT : après `normalise` les soulignés sont devenus des espaces, et le
    // signal « c'est une release scène » aurait disparu.
    let scene_shaped = stem.contains('_');
    let s = normalise(stem);
    let (s, group_version) = strip_groups(&s);
    let s = strip_loose_noise(&s);
    let s = strip_scene_suffix(&s, scene_shaped);
    let s = strip_leading_position(&s);
    (collapse_ws(&s), group_version)
}

fn extract_from(stem: &str) -> (String, String, Option<String>) {
    let (s, group_version) = degrease(stem);

    if s.is_empty() {
        return (String::new(), String::new(), None);
    }

    let mut fields = split_fields(&s);
    // Un `-` esseulé en bout de champ est le résidu d'un groupe retiré
    // (`… - Trippin (Original Mix) -  [320 kbps]`), pas de la ponctuation utile. Seulement en
    // QUEUE : un `-` de tête peut être le titre lui-même (`-ism`).
    for f in fields.iter_mut() {
        *f = f.trim_end_matches([' ', '-', ',', ';']).trim().to_string();
    }
    fields.retain(|f| !f.is_empty());

    // L'ordre compte. Les repères de position et l'année sont retirés AVANT de chercher un champ
    // répété : dans `2003 - Force Feeling - 01 - Maetrik - Force Feeling`, tant que l'année est là
    // le champ répété semble être en position 1 (donc « l'artiste redit ») alors qu'il est
    // l'album. Une fois l'année et le numéro ôtés, la position dit la vérité.
    fields.retain(|f| !is_positional_field(f) && !is_year_field(f));

    // Champ répété : deux formes opposées, distinguées par la POSITION du jumeau.
    //  - jumeau en tête  → l'album porte le même nom que le morceau, et c'est la QUEUE qui est le
    //    titre : on retire la tête (`Krafty - B - Paul Rogers - Krafty`).
    //  - jumeau ailleurs → c'est l'artiste redit en fin de nom par la compilation : on retire la
    //    queue (`Album - Duji - Rena - Duji`).
    if fields.len() >= 3 {
        if let Some(last_key) = fields.last().map(|f| loose_key(f)) {
            if !last_key.is_empty() {
                if fields.first().map(|f| loose_key(f)) == Some(last_key.clone()) {
                    fields.remove(0);
                } else if fields[1..fields.len() - 1]
                    .iter()
                    .any(|f| loose_key(f) == last_key)
                {
                    fields.pop();
                }
            }
        }
    }

    // Un champ d'un ou deux caractères n'est jamais un vrai titre : c'est un résidu de découpage
    // (`…--Fi-LOPZUP`). Le laisser en queue enverrait « Fi » à Discogs comme titre.
    while fields.len() >= 2
        && fields
            .last()
            .map(|f| f.chars().filter(|c| c.is_alphanumeric()).count() <= 2)
            .unwrap_or(false)
    {
        fields.pop();
    }

    // Un label en queue (`- Distants Records`) n'est pas un titre.
    while fields.len() >= 3 && fields.last().map(|f| is_label_field(f)).unwrap_or(false) {
        fields.pop();
    }
    if fields.is_empty() {
        return (String::new(), String::new(), None);
    }

    // Une version peut occuper son propre champ (`Neighbour - Ordinary Unusual - Original Mix`).
    let mut field_version: Option<String> = None;
    if fields.len() >= 3 {
        if let Some(last) = fields.last() {
            if is_version_content(last) && last.split_whitespace().count() <= 4 {
                field_version = Some(last.clone());
                fields.pop();
            }
        }
    }

    let (artist, title) = match fields.len() {
        0 => (String::new(), String::new()),
        1 => (String::new(), fields[0].clone()),
        2 => (fields[0].clone(), fields[1].clone()),
        // ≥3 : le premier champ est l'album (compilations et rips de vinyle le mettent en tête).
        _ => (fields[1].clone(), fields[2..].join(" - ")),
    };

    // Le code de face précède parfois le TITRE (`Magnetic Disorder -  A. The Observer`), y compris
    // sans chiffre. On ne l'applique jamais à l'artiste : `A. Jas` est un vrai nom.
    let title = strip_leading_side_opt(&title, false)
        .map(|t| t.to_string())
        .unwrap_or(title);

    let (title, phrase_version) = split_trailing_version_phrase(&title);
    let version = group_version.or(field_version).or(phrase_version);

    let title = title.trim().to_string();
    let artist = artist.trim().to_string();
    if is_generic_title(&title) {
        return (String::new(), String::new(), None);
    }
    (artist, title, version)
}

// ---------------------------------------------------------------------------------------------
// Étage 5 — le dossier comme source d'artiste
// ---------------------------------------------------------------------------------------------

/// Extrait un artiste du nom de dossier, ou rien.
///
/// Rendre `None` est le comportement par DÉFAUT et le plus important : les dossiers les plus
/// peuplés de la bibliothèque mesurée (`2_040924` avec 524 pistes, `complete` avec 138) ne portent
/// aucune information. Un minage optimiste y injecterait le même faux artiste dans des centaines
/// de requêtes.
fn mine_folder_artist(folder: &str) -> Option<String> {
    if folder.trim().is_empty() {
        return None;
    }
    let s = normalise(folder);
    let (s, _) = strip_groups(&s);
    let s = strip_loose_noise(&s);
    let s = strip_scene_suffix(&s, folder.contains('_'));
    let s = collapse_ws(&s);
    if s.is_empty() {
        return None;
    }
    let mut fields: Vec<String> = split_fields(&s);
    fields.retain(|f| !is_positional_field(f) && !is_year_field(f) && !is_label_field(f));
    // Il FAUT au moins deux champs : un dossier d'un seul champ (`complete`, `rever`,
    // `Jay Tripwire`) est indiscernable d'un nom d'artiste, et se tromper coûte tout le dossier.
    if fields.len() < 2 {
        return None;
    }
    let cand = fields[0].trim();
    if !is_plausible_artist(cand) {
        return None;
    }
    Some(cand.to_string())
}

/// Filtre de plausibilité : au moins une lettre, pas uniquement des chiffres, longueur raisonnable.
fn is_plausible_artist(s: &str) -> bool {
    let t = s.trim();
    if t.len() < 2 || t.chars().count() > 60 {
        return false;
    }
    if !t.chars().any(|c| c.is_alphabetic()) {
        return false;
    }
    if is_generic_title(t) {
        return false;
    }
    true
}

// ---------------------------------------------------------------------------------------------
// Étage 6 — la cascade
// ---------------------------------------------------------------------------------------------

/// Replie les accents et la ponctuation pour un essai ultime (`Béatrice` → `beatrice`).
/// `naming::name_key` fait déjà exactement ça pour la déduplication — on le réutilise plutôt que
/// d'entretenir une seconde table de repli.
fn folded(s: &str) -> String {
    crate::naming::name_key("", s)
}

/// Construit les essais du plus spécifique au plus dégradé.
///
/// La garde qui manquait le plus n'est pas ici mais dans `discogs.rs` : le repli historique exigeait
/// un artiste non vide, ce qui excluait exactement la population la plus sale. La cascade ne
/// suppose jamais qu'un champ est rempli.
fn build_ladder(
    artist: &str,
    title: &str,
    version: Option<&str>,
    folder: &str,
    stem: &str,
) -> Vec<Attempt> {
    let mut out: Vec<Attempt> = Vec::new();
    let mut push = |q: String, label: &'static str| {
        let q = collapse_ws(&q);
        // Une requête de moins de trois caractères alphanumériques ne cherche rien : elle
        // ramènerait la moitié de Discogs. Cas réel : un découpage raté sur
        // `02-Retiro_An-2_Fluent_Remix_` produit le titre « 2 », et l'essai « 2 » consommerait une
        // des trois marches disponibles pour du bruit.
        if q.chars().filter(|c| c.is_alphanumeric()).count() < 3 {
            return;
        }
        if out.iter().any(|a| a.q.eq_ignore_ascii_case(&q)) {
            return;
        }
        out.push(Attempt { q, label });
    };

    // « Tous les mots » AVANT le repli sur les seuls champs extraits : quand le découpage s'est
    // trompé, les champs sont faux mais les mots, eux, sont bons. Placé après les essais
    // structurés seulement quand ceux-ci existent (voir plus bas) — ici il ouvre la marche pour un
    // nom qu'on n'a pas su découper du tout.
    let words = all_words(stem);
    if title.is_empty() {
        push(words, "tous les mots");
        return out;
    }

    match (artist.is_empty(), version) {
        (false, Some(v)) => {
            push(format!("{artist} {title} {v}"), "artiste+titre+version");
            push(format!("{artist} {title}"), "artiste+titre");
        }
        (false, None) => {
            push(format!("{artist} {title}"), "artiste+titre");
        }
        (true, Some(v)) => {
            push(format!("{title} {v}"), "titre+version");
            push(title.to_string(), "titre");
        }
        (true, None) => {
            push(title.to_string(), "titre");
        }
    }
    push(title.to_string(), "titre");
    // Filet : si le titre extrait a perdu des mots présents dans le nom (mauvais découpage), cet
    // essai les rattrape tous. `push` déduplique, donc il ne coûte rien quand le découpage
    // était bon.
    push(words, "tous les mots");

    let f = folded(title);
    if !f.is_empty() && !f.eq_ignore_ascii_case(title) {
        let fa = folded(artist);
        if !fa.is_empty() {
            push(format!("{fa} {f}"), "replie");
        } else {
            push(f, "replie");
        }
    }

    // Dernier recours : l'album miné du dossier resitue une piste dont le titre est trop commun.
    if let Some(album) = mine_folder_album(folder) {
        if !artist.is_empty() {
            push(format!("{artist} {album}"), "artiste+album");
        } else {
            push(format!("{title} {album}"), "titre+album");
        }
    }
    out
}

/// Deuxième champ exploitable du dossier (`(SOMA 21) Slam-Snapshots` → `Snapshots`).
fn mine_folder_album(folder: &str) -> Option<String> {
    if folder.trim().is_empty() {
        return None;
    }
    let s = normalise(folder);
    let (s, _) = strip_groups(&s);
    let s = strip_scene_suffix(&strip_loose_noise(&s), folder.contains('_'));
    let mut fields: Vec<String> = split_fields(&collapse_ws(&s));
    fields.retain(|f| !is_positional_field(f) && !is_year_field(f) && !is_label_field(f));
    if fields.len() < 2 {
        return None;
    }
    let cand = fields[1].trim().to_string();
    if is_plausible_artist(&cand) {
        Some(cand)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_a_clean_name_intact() {
        let t = build("Subsound - Universal Sky", "(2002) The Universal Sky");
        assert_eq!(t.artist, "Subsound");
        assert_eq!(t.title, "Universal Sky");
        assert_eq!(t.version, None);
    }

    #[test]
    fn catalog_bracket_no_longer_kills_the_whole_name() {
        // Le motif du plus gros dossier de la bibliothèque mesurée (524 pistes) : aujourd'hui
        // `naming::parse_filename` rejette tout le nom sur le seul `[`.
        let t = build("[BU 002] DJ Gregory - Freeze", "2_040924");
        assert_eq!(t.artist, "DJ Gregory");
        assert_eq!(t.title, "Freeze");
    }

    #[test]
    fn download_dedup_suffix_is_not_a_version() {
        // `(1)` est un suffixe de doublon de téléchargement. Le lire comme une version enverrait
        // « U-Too 1 » à Discogs.
        let t = build("Demarkus Lewis - U-Too [www.slider.kz] (1)", "complete");
        assert_eq!(t.artist, "Demarkus Lewis");
        assert_eq!(t.title, "U-Too");
        assert_eq!(t.version, None);
    }

    #[test]
    fn intraword_hyphen_is_not_a_separator() {
        let t = build("06 rene breitbarth - sci-fi", "complete");
        assert_eq!(t.artist, "rene breitbarth");
        assert_eq!(t.title, "sci-fi");
    }

    #[test]
    fn scene_suffix_never_eats_a_real_title() {
        // `-ism` est indiscernable d'un tag de groupe SAUF par l'absence de matière devant.
        let t = build("10. -ism", "Cherry Bomb 1995");
        assert_eq!(t.title, "-ism");
    }

    #[test]
    fn folder_supplies_the_missing_artist() {
        let t = build("A1-Stepback", "(SOMA 21) Slam-Snapshots");
        assert_eq!(t.artist, "Slam");
        assert_eq!(t.title, "Stepback");
    }

    #[test]
    fn a_useless_folder_injects_nothing() {
        // Garde-fou : `2_040924` porte 524 pistes. Un minage optimiste y mettrait le même faux
        // artiste dans 524 requêtes.
        assert_eq!(mine_folder_artist("2_040924"), None);
        assert_eq!(mine_folder_artist("complete"), None);
        assert_eq!(mine_folder_artist("Jay Tripwire"), None);
    }

    #[test]
    fn ladder_degrades_and_never_requires_an_artist() {
        let t = build("01 Give U Love (Deep Mix)", "complete");
        assert_eq!(t.artist, "");
        assert_eq!(t.title, "Give U Love");
        assert_eq!(t.version.as_deref(), Some("Deep Mix"));
        let labels: Vec<&str> = t.ladder.iter().map(|a| a.label).collect();
        assert!(
            labels.contains(&"titre+version") && labels.contains(&"titre"),
            "sans artiste, la cascade doit quand même proposer des essais : {labels:?}"
        );
    }

    /// Les deux cas que le découpage en champs ne résout pas. Antoine a dit ce qu'il taperait pour
    /// les trouver — et c'est ça, le vrai critère : la cascade doit contenir cette requête, même
    /// quand on ne sait pas dire lequel des mots est l'artiste.
    #[test]
    fn ladder_contains_what_a_human_would_type_even_when_field_split_fails() {
        for (stem, folder, expected) in [
            (
                "2-Gunne-What-I-Like--Fi-LOPZUP",
                "complete",
                "gunne what i like",
            ),
            (
                "02-Retiro_An-2_Fluent_Remix_",
                "complete",
                "retiro an 2 fluent",
            ),
        ] {
            let t = build(stem, folder);
            let found = t.ladder.iter().any(|a| {
                let q = a.q.to_lowercase();
                expected
                    .split_whitespace()
                    .all(|w| q.split_whitespace().any(|qw| qw == w))
            });
            assert!(
                found,
                "aucun essai ne porte les mots {expected:?} pour {stem:?}\n  cascade: {:?}",
                t.ladder.iter().map(|a| &a.q).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn ladder_has_no_duplicate_queries() {
        for c in crate::search_corpus::CASES {
            let t = build(c.stem, c.folder);
            let mut seen: Vec<String> = Vec::new();
            for a in &t.ladder {
                let k = a.q.to_lowercase();
                assert!(
                    !seen.contains(&k),
                    "essai duplique {:?} pour {:?}",
                    a.q,
                    c.stem
                );
                seen.push(k);
            }
        }
    }

    #[test]
    fn build_is_total_and_never_panics() {
        // Entrées hostiles : le module tourne sur des noms de fichiers arbitraires venus du disque.
        for s in [
            "",
            "   ",
            "-",
            "---",
            "()",
            "[",
            "(((((",
            "]]]]",
            "( - ) - ( - )",
            "\u{2013}\u{2013}\u{2013}",
            "________",
            "01",
            "A1",
            "(1)",
            "\u{1F600} - \u{1F600}",
        ] {
            let t = build(s, s);
            let _ = (t.artist, t.title, t.version, t.ladder);
        }
    }
}
