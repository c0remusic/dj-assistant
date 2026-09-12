//! Corpus de noms de fichiers SALES, tirés d'une vraie bibliothèque, avec pour chacun
//! l'artiste/titre/version attendus. C'est l'ÉTALON du chantier « recherche Discogs sur noms
//! sales » (`docs/superpowers/changes/2026-07-28-discogs-dirty-names/design.md`, tranche T1).
//!
//! Pourquoi un corpus committé plutôt que des cas inventés au fil de l'eau : la moitié des noms
//! de la bibliothèque mesurée (1 355 / 2 714 le 2026-07-28) est rejetée par `naming::parse_filename`,
//! et aucun test existant ne portait de nom réellement sale — les exemples des tests de `naming.rs`
//! se limitaient à `01_audio_320` et `Title (Vinyl Rip)`. Sans étalon, toute amélioration du
//! nettoyage est invérifiable : on ne peut ni prouver un gain, ni détecter une régression sur un
//! motif qu'on ne pensait plus à tester.
//!
//! **Chaque entrée est un motif verrouillé, pas un échantillon statistique.** Le champ `note` dit
//! ce que le cas protège. Ajouter un cas quand on rencontre un motif nouveau ; ne jamais en retirer
//! un pour faire passer un test.
//!
//! Les attendus sont étiquetés à la MAIN (jugement humain sur ce que Discogs devrait recevoir),
//! pas générés par le code testé — sinon le test ne mesurerait que sa propre cohérence.
//!
//! Provenance : `%APPDATA%\com.sift.app\sift.db`, 2 714 pistes, 2026-07-28. Les noms sont
//! reproduits à l'octet près, y compris espaces doubles, espaces finaux, tiret demi-cadratin (–),
//! tiret U+2010 (‐), apostrophe typographique (’) et barre de fraction (⁄) — ces caractères SONT
//! le piège, les normaliser ici viderait le corpus de son intérêt.

/// Un cas du corpus. `artist`/`title` vides = rien n'est dérivable de cette source (le nom ET le
/// dossier sont muets) ; c'est un attendu légitime, pas un trou à combler.
#[derive(Debug)]
pub struct Case {
    /// Nom du dossier parent SEUL (jamais le chemin complet) — deuxième source d'artiste.
    pub folder: &'static str,
    /// Nom de fichier sans extension.
    pub stem: &'static str,
    pub artist: &'static str,
    pub title: &'static str,
    pub version: Option<&'static str>,
    /// Le motif que ce cas verrouille. Sert au message d'échec : un test qui casse doit dire
    /// QUEL motif a régressé, pas seulement quel index.
    pub note: &'static str,
}

pub const CASES: &[Case] = &[
    // ---------- face vinyle en tête ----------
    Case {
        folder: "(2002) The Universal Sky",
        stem: "A1. Subsound - Universal Sky",
        artist: "Subsound",
        title: "Universal Sky",
        version: None,
        note: "face vinyle 'A1.' en tête, sinon nom propre",
    },
    Case {
        folder: "(2002) The Universal Sky",
        stem: "B2. Subsound - Electronix",
        artist: "Subsound",
        title: "Electronix",
        version: None,
        note: "face vinyle 'B2.' en tête",
    },
    Case {
        folder: "(2002) The Universal Sky",
        stem: "Subsound - Universal Sky",
        artist: "Subsound",
        title: "Universal Sky",
        version: None,
        note: "TÉMOIN: nom déjà propre, ne doit pas se dégrader",
    },
    Case {
        folder: "(sk029) The Persuader - City Of Islands (1998)",
        stem: "(a1) The Persuader - Djurgardsbron",
        artist: "The Persuader",
        title: "Djurgardsbron",
        version: None,
        note: "face vinyle entre parenthèses '(a1)' en tête",
    },
    Case {
        folder: "(SOMA 21) Slam-Snapshots",
        stem: "A1-Stepback",
        artist: "Slam",
        title: "Stepback",
        version: None,
        note: "face collée 'A1-', artiste UNIQUEMENT dans le dossier '(LABEL NN) Artiste-Album'",
    },
    Case {
        folder: "(SOMA 21) Slam-Snapshots",
        stem: "C2-Stepback 2",
        artist: "Slam",
        title: "Stepback 2",
        version: None,
        note: "face collée + titre finissant par un chiffre (ne pas confondre avec un numéro)",
    },
    Case {
        folder: "(SOMA 146) Tony Thomas-Good Fortune  Jump",
        stem: "A1-Good Fortune (DJ Hal's Lunar Love Mix)",
        artist: "Tony Thomas",
        title: "Good Fortune",
        version: Some("DJ Hal's Lunar Love Mix"),
        note: "artiste du dossier + version conservée (apostrophe droite dans la version)",
    },
    Case {
        folder: "2_040924",
        stem: "[BR 95004] A1 Baron Noir - Paris",
        artist: "Baron Noir",
        title: "Paris",
        version: None,
        note: "catalogue en crochets PUIS face sans ponctuation 'A1 '",
    },
    Case {
        folder: "2_040924",
        stem: "[002] Magnetic Disorder -  A. The Observer",
        artist: "Magnetic Disorder",
        title: "The Observer",
        version: None,
        note: "face 'A.' APRÈS le séparateur, côté titre + double espace",
    },
    // ---------- numéro de piste en tête ----------
    Case {
        folder: "complete",
        stem: "01 Awaken Abyss",
        artist: "",
        title: "Awaken Abyss",
        version: None,
        note: "numéro seul, AUCUN artiste dérivable (dossier muet) - artiste vide est l'attendu",
    },
    Case {
        folder: "complete",
        stem: "05 Give U Love",
        artist: "",
        title: "Give U Love",
        version: None,
        note: "numéro seul, titre court",
    },
    Case {
        folder: "complete",
        stem: "01 Give U Love (Deep Mix)",
        artist: "",
        title: "Give U Love",
        version: Some("Deep Mix"),
        note: "sans artiste MAIS avec version - aujourd'hui la version est jetée (naming.rs:135)",
    },
    Case {
        folder: "complete",
        stem: "01 Music For The Soul (Dob_s Mix)",
        artist: "",
        title: "Music For The Soul",
        version: Some("Dob s Mix"),
        note: "souligné À L'INTÉRIEUR de la version (apostrophe perdue à l'encodage du nom)",
    },
    Case {
        folder: "complete",
        stem: "01-14 Rainforest (Rare electro mix)",
        artist: "",
        title: "Rainforest",
        version: Some("Rare electro mix"),
        note: "double numéro disque-piste '01-14' (ne pas lire 01 comme artiste)",
    },
    Case {
        folder: "complete",
        stem: "01 Olsvanger - The Triss",
        artist: "Olsvanger",
        title: "The Triss",
        version: None,
        note: "numéro + séparateur normal",
    },
    Case {
        folder: "complete",
        stem: "07 misc. - frequenztrager",
        artist: "misc.",
        title: "frequenztrager",
        version: None,
        note: "artiste finissant par un point (ne pas confondre avec une face vinyle)",
    },
    Case {
        folder: "2013 - Split EP [RB040, WEB]",
        stem: "01 - Roman IV - Happy",
        artist: "Roman IV",
        title: "Happy",
        version: None,
        note: "'NN - Artiste - Titre' : le numéro est le 1er champ, PAS l'artiste",
    },
    Case {
        folder: "20_20 Vision\u{2044}VIS050 Ralph Lawson - Visionaries Volume One FLAC",
        stem: "01. Wolf n' Flow - Activate",
        artist: "Wolf n' Flow",
        title: "Activate",
        version: None,
        note: "dossier contenant une barre de fraction U+2044 (substitut de séparateur de chemin)",
    },
    Case {
        folder: "complete",
        stem: "1-05 - Green Thumb vs. JV - Grand Theft Vinyl (JV Mix)",
        artist: "Green Thumb vs. JV",
        title: "Grand Theft Vinyl",
        version: Some("JV Mix"),
        note: "'D-NN - Artiste vs. Artiste - Titre (Version)'",
    },
    Case {
        folder: "1 prog 90's 5",
        stem: "2-07. Alaska feat. Neve - Forbidden (Sumantri's Tribal Dub)",
        artist: "Alaska feat. Neve",
        title: "Forbidden",
        version: Some("Sumantri's Tribal Dub"),
        note: "'feat.' fait partie de l'artiste et doit SURVIVRE",
    },
    Case {
        folder: "1 prog 90's 5",
        stem: "132.01--A. Jas - Dirty Carnival Music (Original)",
        artist: "A. Jas",
        title: "Dirty Carnival Music",
        version: Some("Original"),
        note: "numéro à 3 chiffres + point + double tiret, artiste avec initiale 'A. '",
    },
    // ---------- séparateurs exotiques ----------
    Case {
        folder: "complete",
        stem: "01_dj_hal_and_jay_thomas_-_dont_stop_(tony_thomas_remix)",
        artist: "dj hal and jay thomas",
        title: "dont stop",
        version: Some("tony thomas remix"),
        note: "séparateur '_-_' + tout en souligné",
    },
    Case {
        folder: "complete",
        stem: "01_jeff_bennett_-_falling_up-sq",
        artist: "jeff bennett",
        title: "falling up",
        version: None,
        note: "séparateur '_-_' + suffixe scene '-sq' à retirer",
    },
    Case {
        folder: "complete",
        stem: "02-maetrik--force_feeling_(decomposed_subsonic_remix)-dh",
        artist: "maetrik",
        title: "force feeling",
        version: Some("decomposed subsonic remix"),
        note: "séparateur '--' + suffixe scene '-dh'",
    },
    Case {
        folder: "complete",
        stem: "02-vince_watson-method_of_emotion-7d468690",
        artist: "vince watson",
        title: "method of emotion",
        version: None,
        note: "séparateur '-' collé + hash hexadécimal 8 chiffres en queue",
    },
    Case {
        folder: "complete",
        stem: "03-Janeret-Mush_Vitamina",
        artist: "Janeret",
        title: "Mush Vitamina",
        version: None,
        note: "'NN-Artiste-Titre' entièrement collé",
    },
    Case {
        folder: "complete",
        stem: "11-Maetrik-Force Feeling",
        artist: "Maetrik",
        title: "Force Feeling",
        version: None,
        note: "'NN-Artiste-Titre' avec espaces dans le titre",
    },
    Case {
        folder: "complete",
        stem: "12-Maetrik-Force Feeling (Decomposed Subsonic Rmx)",
        artist: "Maetrik",
        title: "Force Feeling",
        version: Some("Decomposed Subsonic Rmx"),
        note: "même forme + version ('Rmx' abrégé)",
    },
    Case {
        folder: "1 prog 90's 5",
        stem: "DJ RAGE.-Waiting(Antoine Clamaran remix)(mp3) ",
        artist: "DJ RAGE",
        title: "Waiting",
        version: Some("Antoine Clamaran remix"),
        note: "séparateur '.-' collé + version collée + suffixe '(mp3)' + ESPACE FINAL",
    },
    Case {
        folder: "1 prog 90's 5",
        stem: "Laurent Garnier - Crispy Bacon (King Unique Remix)(mp3) ",
        artist: "Laurent Garnier",
        title: "Crispy Bacon",
        version: Some("King Unique Remix"),
        note: "suffixe '(mp3)' collé à la version + espace final",
    },
    Case {
        folder: "1 prog 90's 5",
        stem: "CJ Art \u{2013} Acedia (Original mix) (DIS009) \u{2013} Distants Records",
        artist: "CJ Art",
        title: "Acedia",
        version: Some("Original mix"),
        note: "tiret DEMI-CADRATIN comme séparateur + catalogue et label en queue",
    },
    Case {
        folder: "complete",
        stem: "2-Gunne-What-I-Like--Fi-LOPZUP",
        artist: "Gunne",
        title: "What I Like",
        version: None,
        note: "release scene tout en tirets, groupe 'LOPZUP' en queue",
    },
    Case {
        folder: "complete",
        stem: "06 rene breitbarth - sci-fi",
        artist: "rene breitbarth",
        title: "sci-fi",
        version: None,
        note: "PIÈGE: 'sci-fi' a un tiret INTRAMOT qui n'est PAS un séparateur",
    },
    Case {
        folder: "2_040924",
        stem: "[0012] QA 0-127 - Fiction",
        artist: "QA 0-127",
        title: "Fiction",
        version: None,
        note: "PIÈGE: artiste contenant chiffres ET tiret intramot, après un catalogue",
    },
    // ---------- crochets ----------
    Case {
        folder: "2_040924",
        stem: "[BU 002] DJ Gregory - Freeze",
        artist: "DJ Gregory",
        title: "Freeze",
        version: None,
        note: "catalogue en tête - motif du plus gros dossier (524 pistes), rejeté en bloc aujourd'hui",
    },
    Case {
        folder: "2_040924",
        stem: "[12 BC-001] Andy Mowat - Trippin",
        artist: "Andy Mowat",
        title: "Trippin",
        version: None,
        note: "catalogue avec espace ET tiret interne",
    },
    Case {
        folder: "2_040924",
        stem: "[BARON - 001] Baron Feat. Daddy E - Boomselecter",
        artist: "Baron Feat. Daddy E",
        title: "Boomselecter",
        version: None,
        note: "PIÈGE: le catalogue contient ' - ', le séparateur naïf coupe dedans",
    },
    Case {
        folder: "2_040924",
        stem: "[CHAIR-006] Daniel Lui - Untitled A",
        artist: "Daniel Lui",
        title: "Untitled A",
        version: None,
        note: "'Untitled A' est un VRAI titre ici, pas un nom manquant",
    },
    Case {
        folder: "1 prog 90's 5",
        stem: "[101] Jaimy & Kenny D. - Like A Bitch",
        artist: "Jaimy & Kenny D.",
        title: "Like A Bitch",
        version: None,
        note: "catalogue numérique court + artiste finissant par un point",
    },
    Case {
        folder: "2_040924",
        stem: "(spm007] Cle Acklin - My Face (Original Dirty Mix)",
        artist: "Cle Acklin",
        title: "My Face",
        version: Some("Original Dirty Mix"),
        note: "PIÈGE: crochet DÉPAREILLÉ '(...]' - le retrait par paires échoue",
    },
    Case {
        folder: "complete",
        stem: "01 - abduction [original]",
        artist: "",
        title: "abduction",
        version: Some("original"),
        note: "crochet TERMINAL = version (par opposition au crochet initial = catalogue)",
    },
    Case {
        folder: "2016 - Korsakow - Abduction",
        stem: "02. Abduction [benonedit]",
        artist: "Korsakow",
        title: "Abduction",
        version: Some("benonedit"),
        note: "même titre, artiste récupéré du dossier 'ANNÉE - Artiste - Album'",
    },
    Case {
        folder: "complete",
        stem: "03 Freaky Chakra - Sean Q6 , Out In The Shed [Freaky Chakra Mix]",
        artist: "Freaky Chakra",
        title: "Sean Q6 , Out In The Shed",
        version: Some("Freaky Chakra Mix"),
        note: "crochet terminal = version, titre contenant une virgule",
    },
    Case {
        folder: "complete",
        stem: "03. Neighbour - [Go Ahead EP] Ordinary Unusual - Original Mix",
        artist: "Neighbour",
        title: "Ordinary Unusual",
        version: Some("Original Mix"),
        note: "crochet AU MILIEU (nom d'EP) + version après un ' - ' et non entre parenthèses",
    },
    Case {
        folder: "complete",
        stem: "Demarkus Lewis - U-Too [www.slider.kz] (1)",
        artist: "Demarkus Lewis",
        title: "U-Too",
        version: None,
        note: "PIÈGE MAJEUR: '(1)' est un suffixe de doublon de téléchargement, PAS une version",
    },
    // ---------- URL / débit / bruit de source ----------
    Case {
        folder: "Francesco Del Garda's Track IDs",
        stem: "Oris Jay ft. Delsena - Trippin (Original Mix) -  [320 kbps]",
        artist: "Oris Jay ft. Delsena",
        title: "Trippin",
        version: Some("Original Mix"),
        note: "'[320 kbps]' en queue + 'ft.' a conserver dans l'artiste",
    },
    Case {
        folder: "flat 15112025",
        stem: "Slippy G - Pixel Waterfall Original Mix-www.groovytunes.org",
        artist: "Slippy G",
        title: "Pixel Waterfall",
        version: Some("Original Mix"),
        note: "URL COLLÉE à la version, sans parenthèses autour de 'Original Mix'",
    },
    Case {
        folder: "Francesco Del Garda",
        stem: "John Dimas - Self Control (Original Mix) www.promo-sound.com",
        artist: "John Dimas",
        title: "Self Control",
        version: Some("Original Mix"),
        note: "URL en queue séparée par une espace",
    },
    Case {
        folder: "Francesco Del Garda's Track IDs",
        stem: "100Hz - The Field (Original Mix) - www.djsoundtop.com",
        artist: "100Hz",
        title: "The Field",
        version: Some("Original Mix"),
        note: "PIÈGE: l'artiste '100Hz' contient un token de qualité ('hz') et des chiffres",
    },
    Case {
        folder: "1 prog 90's 5",
        stem: "Dane Jolly - Boomslang (Super Fly's Vision Tool Mix)-001",
        artist: "Dane Jolly",
        title: "Boomslang",
        version: Some("Super Fly's Vision Tool Mix"),
        note: "suffixe numérique '-001' en queue",
    },
    Case {
        folder: "1 prog 90's 5",
        stem: "Dane Jolly - Boomslang (Super Fly's Vision Tool Mix)",
        artist: "Dane Jolly",
        title: "Boomslang",
        version: Some("Super Fly's Vision Tool Mix"),
        note: "TÉMOIN: même piste SANS le suffixe, les deux doivent donner le même résultat",
    },
    Case {
        folder: "1 prog 90's 5",
        stem: "B1. Subway Baby - Tribes Of Khan Gala (Homerun Mix)(mp3) ",
        artist: "Subway Baby",
        title: "Tribes Of Khan Gala",
        version: Some("Homerun Mix"),
        note: "face vinyle + version + '(mp3)' + espace final cumules",
    },
    Case {
        folder: "complete",
        stem: "01-onionz_and_the_dcl_project-chili_con_huevos_(pepo_remix)-ccat",
        artist: "onionz and the dcl project",
        title: "chili con huevos",
        version: Some("pepo remix"),
        note: "numéro + souligné + suffixe scene '-ccat'",
    },
    Case {
        folder: "1 prog 90's 5",
        stem: "26-pablo_briales_and_ruben_cano-delirium_(sergio_fernandez_remix)-scmt",
        artist: "pablo briales and ruben cano",
        title: "delirium",
        version: Some("sergio fernandez remix"),
        note: "artiste long en souligné + suffixe scene '-scmt'",
    },
    Case {
        folder: "complete",
        stem: "01_infunktuation_-_feel_real_good_(club_version)-idc",
        artist: "infunktuation",
        title: "feel real good",
        version: Some("club version"),
        note: "trois pistes du même titre ne diffèrent QUE par la version - elle est décisive",
    },
    Case {
        folder: "complete",
        stem: "02_infunktuation_-_feel_real_good_(dub_version)-idc",
        artist: "infunktuation",
        title: "feel real good",
        version: Some("dub version"),
        note: "même titre, version différente (voir cas précédent)",
    },
    Case {
        folder: "complete",
        stem: "03_infunktuation_-_feel_real_good_(obscure_version)-idc",
        artist: "infunktuation",
        title: "feel real good",
        version: Some("obscure version"),
        note: "même titre, 3e version (voir deux cas précédents)",
    },
    Case {
        folder: "complete",
        stem: "05-jas-soul_doing_dishes_(luke_fair_mix)",
        artist: "jas",
        title: "soul doing dishes",
        version: Some("luke fair mix"),
        note: "artiste très court (3 lettres) collé entre deux tirets",
    },
    Case {
        folder: "complete",
        stem: "02-Jay_Welsh_-_Weird_Noises_(Northface_remix_2)",
        artist: "Jay Welsh",
        title: "Weird Noises",
        version: Some("Northface remix 2"),
        note: "version finissant par un chiffre",
    },
    Case {
        folder: "complete",
        stem: "02-Retiro_An-2_Fluent_Remix_",
        artist: "Retiro",
        title: "An-2",
        version: Some("Fluent Remix"),
        note: "PIÈGE: titre 'An-2' à tiret intramot, version sans parenthèses, souligné FINAL",
    },
    Case {
        folder: "complete",
        stem: "02. Hill & Funez - Don't Hesitate (Dub Version)",
        artist: "Hill & Funez",
        title: "Don't Hesitate",
        version: Some("Dub Version"),
        note: "esperluette dans l'artiste + apostrophe dans le titre",
    },
    Case {
        folder: "complete",
        stem: "02 - Zwicker Meets James Teipdeck - Homage to XY",
        artist: "Zwicker Meets James Teipdeck",
        title: "Homage to XY",
        version: None,
        note: "TÉMOIN propre pour comparer avec la variante scene ci-dessous",
    },
    Case {
        folder: "complete",
        stem: "02-zwicker_meets_james_teipdeck--homage_to_xy-805453c5",
        artist: "zwicker meets james teipdeck",
        title: "homage to xy",
        version: None,
        note: "MÊME piste que le témoin ci-dessus, forme scene - doit converger",
    },
    Case {
        folder: "1 prog 90's 5",
        stem: "Ozgur Ozkan - Too Late (CJ Art's Too Tribal Mix)",
        artist: "Ozgur Ozkan",
        title: "Too Late",
        version: Some("CJ Art's Too Tribal Mix"),
        note: "TÉMOIN: déjà propre, ne doit pas se dégrader",
    },
    // ---------- compilations : l'artiste n'est PAS le premier champ ----------
    Case {
        folder: "(2004) Doin\u{2019} It After Dark, Volume 2",
        stem: "01  - Doin\u{2019} It After Dark, Volume 2 - Duji - Rena - Duji",
        artist: "Duji",
        title: "Rena",
        version: None,
        note: "compil 'NN - Album - Artiste - Titre - Artiste' : artiste en 3e position, repete en queue",
    },
    Case {
        folder: "(2004) Doin\u{2019} It After Dark, Volume 2",
        stem: "02  - Doin\u{2019} It After Dark, Volume 2 - Digs, Woosh & Mr Ski - Rumpfunk (Raw Substance mix) - Digs, Woosh & Mr Ski",
        artist: "Digs, Woosh & Mr Ski",
        title: "Rumpfunk",
        version: Some("Raw Substance mix"),
        note: "même compil, artiste contenant des virgules (le comptage de champs ne suffit pas)",
    },
    Case {
        folder: "(2004) Doin\u{2019} It After Dark, Volume 2",
        stem: "10  - Doin\u{2019} It After Dark, Volume 2 - Markus Enochson feat. E\u{2010}Man - Sweetlove (Alex Phountzi remix) - Markus Enochson feat. E\u{2010}Man",
        artist: "Markus Enochson feat. E\u{2010}Man",
        title: "Sweetlove",
        version: Some("Alex Phountzi remix"),
        note: "même compil + 'feat.' + tiret U+2010 dans le nom d'artiste",
    },
    Case {
        folder: "(2004) Doin\u{2019} It After Dark, Volume 2",
        stem: "12  - Doin\u{2019} It After Dark, Volume 2 - Sam Paganini - Gonna Make You Sweat - Sam Paganini",
        artist: "Sam Paganini",
        title: "Gonna Make You Sweat",
        version: None,
        note: "même compil sans version - la répétition finale de l'artiste est le seul repère",
    },
    Case {
        folder: "(2005) Lo Rez - You Don't Win Friends With Salad (Vinyl-FLAC)",
        stem: "[FAR11 - 2005] You Don't Win Friends With Salad - A2 - Lo Rez - LS11",
        artist: "Lo Rez",
        title: "LS11",
        version: None,
        note: "'[CAT - ANNÉE] Album - FACE - Artiste - Titre' : artiste en 3e position",
    },
    Case {
        folder: "1 prog 90's 5",
        stem: "[GU019CD - 2001] Global Underground 019 (UnMixed) - 8 - Photek - Mine To Give (Satoshi Tomiie Dub)",
        artist: "Photek",
        title: "Mine To Give",
        version: Some("Satoshi Tomiie Dub"),
        note: "compil '[CAT - ANNÉE] Album (UnMixed) - NN - Artiste - Titre (Version)'",
    },
    Case {
        folder: "1 prog 90's 5",
        stem: "[SS 014 - 2002] Krafty - B - Paul Rogers - Krafty (Gpal Pandesia Dub Mix)",
        artist: "Paul Rogers",
        title: "Krafty",
        version: Some("Gpal Pandesia Dub Mix"),
        note: "PIÈGE: le titre 'Krafty' est AUSSI le nom de la release, en 1re position",
    },
    Case {
        folder: "(SUR020 - 2001) The Mingers & Mr. G - The Mingers Do Church",
        stem: "(SUR020 - 2001) The Mingers Do Church - B - The Mingers & Mr. G - The Mingers Do Church (Mr G's Not On Sundays Dub)",
        artist: "The Mingers & Mr. G",
        title: "The Mingers Do Church",
        version: Some("Mr G's Not On Sundays Dub"),
        note: "catalogue entre PARENTHÈSES + titre identique à l'album, répété",
    },
    Case {
        folder: "complete",
        stem: "2003 - Force Feeling - 01 - Maetrik - Force Feeling",
        artist: "Maetrik",
        title: "Force Feeling",
        version: None,
        note: "'ANNÉE - Album - NN - Artiste - Titre' : cinq champs, artiste en 4e position",
    },
    // ---------- vraiment sans espoir : l'attendu EST le vide ----------
    Case {
        folder: "02 [2015]",
        stem: "001_Untitled",
        artist: "",
        title: "",
        version: None,
        note: "SANS ESPOIR: rien dans le nom, rien dans le dossier - le vide est l'attendu correct",
    },
    Case {
        folder: "2_040924",
        stem: "[DRAGON002] A1",
        artist: "",
        title: "",
        version: None,
        note: "SANS ESPOIR: catalogue + face, aucun champ nomme",
    },
    Case {
        folder: "The Tracking System",
        stem: "A8",
        artist: "",
        title: "",
        version: None,
        note: "SANS ESPOIR: face vinyle seule",
    },
    Case {
        folder: "Cherry Bomb 1995 Electronics For Dogs (Freedag Nuveux, FDAG CD1)",
        stem: "10. -ism",
        artist: "",
        title: "-ism",
        version: None,
        note: "PIÈGE: '-ism' ressemble à un suffixe scene mais c'est LE titre - ne pas le manger",
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Le corpus doit rester un corpus : des doublons exacts fausseraient tout taux calculé
    /// dessus, et un `note` vide priverait un échec futur de son diagnostic.
    #[test]
    fn corpus_is_well_formed() {
        let mut seen: HashSet<(&str, &str)> = HashSet::new();
        for c in CASES {
            assert!(
                seen.insert((c.folder, c.stem)),
                "doublon (dossier, stem) dans le corpus : {:?} / {:?}",
                c.folder,
                c.stem
            );
            assert!(!c.note.trim().is_empty(), "note vide pour {:?}", c.stem);
            assert!(!c.stem.trim().is_empty(), "stem vide dans le corpus");
            // Un artiste sans titre n'a pas de sens ; l'inverse (titre sans artiste) est légitime.
            if !c.artist.is_empty() {
                assert!(
                    !c.title.is_empty(),
                    "artiste sans titre pour {:?} — attendu incohérent",
                    c.stem
                );
            }
        }
        assert!(
            CASES.len() >= 70,
            "corpus trop maigre pour être un étalon : {} cas",
            CASES.len()
        );
    }

    /// Ce que `naming::reconcile` (état du 2026-07-28, AUCUN tag ni dossier fourni) résout sur le
    /// corpus. MESURÉ, pas décidé — première exécution du test, 77 cas.
    ///
    /// Ces chiffres ne sont pas là pour être satisfaits mais pour être BATTUS par `search_terms`
    /// en T2, et pour qu'une modification de `naming.rs` qui déplace l'aiguille — dans un sens ou
    /// dans l'autre — ne passe pas inaperçue. Les quatre sont figés séparément parce qu'un total
    /// stable peut masquer un gain sur le titre payé par une perte sur l'artiste.
    ///
    /// Lecture du 2026-07-28 : la version est déjà bien extraite (57 %), l'artiste presque jamais
    /// (20 %) — cohérent avec le diagnostic, `extract_trailing_version` est purement syntaxique
    /// alors que l'artiste dépend du portail de rejet.
    const BASELINE_EXACT: usize = 5;
    const BASELINE_ARTIST: usize = 15;
    const BASELINE_TITLE: usize = 17;
    const BASELINE_VERSION: usize = 44;

    /// Rejoue le corpus contre le code ACTUEL. `reconcile` ne reçoit pas de tags (le corpus mesure
    /// l'extraction depuis le nom et le dossier) et ne reçoit pas le dossier du tout — il ne sait
    /// pas le lire, c'est précisément le manque que T3 comblera.
    #[test]
    fn corpus_baseline_against_current_naming() {
        let mut exact = 0usize;
        let mut artist_ok = 0usize;
        let mut title_ok = 0usize;
        let mut version_ok = 0usize;
        let mut failures: Vec<String> = Vec::new();

        for c in CASES {
            let got = crate::naming::reconcile("", "", c.stem);
            let a = got.artist == c.artist;
            let t = got.title == c.title;
            let v = got.version.as_deref() == c.version;
            if a {
                artist_ok += 1;
            }
            if t {
                title_ok += 1;
            }
            if v {
                version_ok += 1;
            }
            if a && t && v {
                exact += 1;
            } else {
                failures.push(format!(
                    "  [{}]\n    stem     {:?}\n    attendu  artiste={:?} titre={:?} version={:?}\n    obtenu   artiste={:?} titre={:?} version={:?}",
                    c.note, c.stem, c.artist, c.title, c.version, got.artist, got.title, got.version
                ));
            }
        }

        let n = CASES.len();
        println!(
            "\n=== CORPUS noms sales — ligne de base (naming::reconcile, sans tags) ===\n\
             cas            : {n}\n\
             exacts         : {exact} ({:.1}%)\n\
             artiste correct: {artist_ok} ({:.1}%)\n\
             titre correct  : {title_ok} ({:.1}%)\n\
             version correcte: {version_ok} ({:.1}%)\n",
            100.0 * exact as f64 / n as f64,
            100.0 * artist_ok as f64 / n as f64,
            100.0 * title_ok as f64 / n as f64,
            100.0 * version_ok as f64 / n as f64,
        );
        for f in failures.iter().take(5) {
            println!("{f}");
        }

        assert_eq!(
            (exact, artist_ok, title_ok, version_ok),
            (
                BASELINE_EXACT,
                BASELINE_ARTIST,
                BASELINE_TITLE,
                BASELINE_VERSION
            ),
            "\nLa ligne de base du corpus a bougé (exact/artiste/titre/version).\n\
             Si c'est voulu (amélioration de naming.rs), mettre les quatre constantes à jour DANS\n\
             LE MÊME changement, en disant pourquoi. Sinon c'est une régression.\n"
        );
    }

    /// Ce que `search_terms::build` résout sur le même corpus. Même rôle que les constantes de
    /// base : un plancher qu'on ne redescend pas sans le dire. Un chiffre qui MONTE est aussi un
    /// échec de test — délibérément : il faut alors constater le gain, vérifier qu'il n'est pas dû
    /// à un attendu relâché, et le figer.
    ///
    /// 75/77 au 2026-07-28. Les deux manqués sont des formes scène réellement ambiguës, laissées
    /// telles quelles DÉLIBÉRÉMENT — les résoudre demanderait des règles si spécifiques qu'elles
    /// abîmeraient des cas voisins :
    ///   - `2-Gunne-What-I-Like--Fi-LOPZUP` : tout en tirets, impossible de savoir où finit
    ///     l'artiste sans connaître le disque.
    ///   - `02-Retiro_An-2_Fluent_Remix_` : le souligné y sert de séparateur de CHAMP alors qu'il
    ///     sert de séparateur de MOT partout ailleurs, et le titre `An-2` contient lui-même un
    ///     tiret.
    ///
    /// Ils restent au corpus : un corpus qui ne garderait que ce qu'on sait résoudre mentirait sur
    /// la difficulté réelle.
    const TERMS_EXACT: usize = 75;
    const TERMS_ARTIST: usize = 75;
    const TERMS_TITLE: usize = 75;
    const TERMS_VERSION: usize = 77;

    #[test]
    fn corpus_against_search_terms() {
        let mut exact = 0usize;
        let (mut artist_ok, mut title_ok, mut version_ok) = (0usize, 0usize, 0usize);
        let mut failures: Vec<String> = Vec::new();

        for c in CASES {
            let got = crate::search_terms::build(c.stem, c.folder);
            let a = got.artist == c.artist;
            let t = got.title == c.title;
            let v = got.version.as_deref() == c.version;
            if a {
                artist_ok += 1;
            }
            if t {
                title_ok += 1;
            }
            if v {
                version_ok += 1;
            }
            if a && t && v {
                exact += 1;
            } else {
                failures.push(format!(
                    "  [{}]\n    dossier  {:?}\n    stem     {:?}\n    attendu  artiste={:?} titre={:?} version={:?}\n    obtenu   artiste={:?} titre={:?} version={:?}",
                    c.note, c.folder, c.stem, c.artist, c.title, c.version,
                    got.artist, got.title, got.version
                ));
            }
        }

        let n = CASES.len();
        println!(
            "\n=== CORPUS noms sales — search_terms::build ===\n\
             cas             : {n}\n\
             exacts          : {exact} ({:.1}%)   [base naming: {BASELINE_EXACT}]\n\
             artiste correct : {artist_ok} ({:.1}%)   [base: {BASELINE_ARTIST}]\n\
             titre correct   : {title_ok} ({:.1}%)   [base: {BASELINE_TITLE}]\n\
             version correcte: {version_ok} ({:.1}%)   [base: {BASELINE_VERSION}]\n",
            100.0 * exact as f64 / n as f64,
            100.0 * artist_ok as f64 / n as f64,
            100.0 * title_ok as f64 / n as f64,
            100.0 * version_ok as f64 / n as f64,
        );
        for f in &failures {
            println!("{f}");
        }

        assert_eq!(
            (exact, artist_ok, title_ok, version_ok),
            (TERMS_EXACT, TERMS_ARTIST, TERMS_TITLE, TERMS_VERSION),
            "\nLe résultat de search_terms sur le corpus a bougé.\n\
             Mettre les quatre constantes à jour DANS LE MÊME changement, en disant pourquoi.\n"
        );
    }
}
