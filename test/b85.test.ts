import { describe, expect, it } from "vitest";
import { decodeB85 } from "../frontend/b85";

// `frontend/b85.ts` est un miroir ÉCRIT À LA MAIN du crate `base85` 2.0.0 utilisé côté
// Rust (`src-tauri/src/b85_bytes.rs`). Les deux implémentations ne partagent aucune ligne :
// rien, dans le compilateur ou dans la CI, ne remarque qu'elles divergent. Et une divergence
// ici est SILENCIEUSE — le décodeur rendrait des octets faux, donc un spectrogramme faux,
// donc un verdict LOSSLESS/FAKE faux, sans une seule erreur nulle part.
//
// Le côté Rust avait déjà prévu ce test : `frozen_vector_matches_the_reference_encoding`
// (`b85_bytes.rs:130`) gèle une chaîne « pour tout décodeur indépendant, e.g. celui du
// frontend ». Elle n'avait jamais eu de lecteur côté frontend. C'est ce fichier.

describe("decodeB85", () => {
  it("décode le vecteur gelé de la référence Rust", () => {
    // Copié depuis `b85_bytes.rs:134`, PAS régénéré depuis `frontend/b85.ts` — c'est tout
    // l'intérêt : la valeur attendue vient de l'autre implémentation. La regénérer depuis
    // le code testé transformerait ce test en tautologie.
    const bytes = decodeB85("009C61O)~M2nh-c3=Iws");
    expect(Array.from(bytes)).toEqual([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);
  });

  it("rend un tableau vide sur la sentinelle « pas de spectrogramme »", () => {
    // `ipc.rs` utilise `mag_db.is_empty()` comme sentinelle « spectrogramme non calculé »,
    // et `b85_bytes.rs` sérialise `vec![]` en `""`. Un décodage non vide ici casserait la
    // lecture du cache (`empty_stays_empty_through_the_string_form`, `b85_bytes.rs:151`).
    expect(decodeB85("")).toHaveLength(0);
  });

  it("échoue bruyamment sur un caractère hors alphabet", () => {
    // Miroir de `rejects_a_character_outside_the_alphabet` (`b85_bytes.rs:160`). L'espace
    // n'est pas dans l'alphabet RFC1924. Le comportement à geler n'est PAS « ça échoue »
    // mais « ça échoue au lieu de rendre des zéros » : un décodage tolérant peindrait un
    // spectrogramme entièrement noir, qui a l'air d'un résultat.
    expect(() => decodeB85("00 9C")).toThrow(/base85 decode: invalid character/);
    // La position doit être citée — sans elle, l'erreur ne situe pas la corruption.
    expect(() => decodeB85("00 9C")).toThrow(/at 2/);
  });

  it("refuse un groupe final d'un seul caractère", () => {
    // 6 caractères = un groupe plein + un reste de 1. Le crate appelle ça `UnexpectedEof` :
    // aucun nombre d'octets ne s'encode sur un seul caractère de reste, donc l'entrée est
    // tronquée. Décoder quand même produirait un tableau plus court que ce que l'appelant
    // attend, et `report-view.ts` lit `mag_db` par `frames * bins` — un tableau court
    // décalerait toute la grille. Deuxième filet depuis le 2026-08-05, pour les troncatures
    // que ce reste-là ne trahit pas (toute longueur multiple de 5) :
    // `assertSpectrogramLength` (`frontend/ipc.ts`), voir `test/spectrogram-length.test.ts`.
    expect(() => decodeB85("009C61")).toThrow(/unexpected end of input/);
  });

  it("préserve la longueur sur un groupe plein", () => {
    // 5 caractères par groupe de 4 octets. Le contrat de taille (`frames * bins`) n'est pas
    // l'affaire du décodeur : il n'a pas les `frames`/`bins` qui le définissent. Il est tenu par
    // son appelant, `assertSpectrogramLength` (`frontend/ipc.ts`) — ce qui, jusqu'au 2026-08-05,
    // n'était vrai de personne : cette phrase décrivait une intention comme un fait.
    expect(decodeB85("009C6")).toHaveLength(4);
    expect(decodeB85("009C61O)~M")).toHaveLength(8);
  });
});

// ────────────────────────────────────────────────────────────────────────────────────────
// Chemin de RESTE — les 2, 3 ou 4 caractères finaux (`b85.ts:61-76`).
//
// Il tourne en production sur presque chaque piste : le spectrogramme fait `frames * bins`
// octets, quasiment jamais aligné sur 4.
//
// TOUTES les valeurs attendues de cette section sortent du crate Rust `base85` 2.0.0 — la
// version épinglée par `src-tauri/Cargo.lock:266-269`, celle que `b85_bytes.rs` appelle.
// Aucune n'a été produite par `frontend/b85.ts` : le test comparerait le code à lui-même.
//
// Régénération — projet cargo jetable HORS du repo (`base85 = "=2.0.0"`), pour ne pas se
// disputer le lock de `src-tauri/target/` avec un `tauri dev` (CLAUDE.md § Commandes).
// Deux formes, selon ce qu'on veut tenir :
//   • round-trip : `base85::encode(&src)`, puis `base85::decode` de la chaîne pour ne garder
//     que ce que le crate lui-même retrouve ;
//   • forgée : `base85::decode("Ir")` sur une chaîne que l'ENCODEUR ne produit jamais. Le
//     décodeur Rust reste la référence pour ces entrées-là aussi — c'est ce qui permet de
//     tenir la constante de padding, voir plus bas.

/** `(i * 37 + 11) as u8`, i = 0..16 — générateur de `round_trips_every_length_and_remainder`
 *  (`b85_bytes.rs:100`). Mêmes entrées que le test Rust, mais avec la chaîne encodée, que
 *  lui calcule sans jamais l'imprimer. */
const SRC = [11, 48, 85, 122, 159, 196, 233, 14, 51, 88, 125, 162, 199, 236, 17, 54, 91];

/** `base85::encode(&SRC[0..n])`, index = n. Les longueurs 1, 2, 3 (mod 4) donnent un reste de
 *  2, 3, 4 caractères ; chacune revient quatre fois sur la plage. */
const RUST_ENCODED = [
  "",
  "3j",
  "3or",
  "3oum",
  "3ouoB",
  "3ouoBp8",
  "3ouoBpTq",
  "3ouoBpTy|",
  "3ouoBpTy}7",
  "3ouoBpTy}7GX",
  "3ouoBpTy}7Ggt",
  "3ouoBpTy}7Ggy5",
  "3ouoBpTy}7Ggy71",
  "3ouoBpTy}7Ggy71#{",
  "3ouoBpTy}7Ggy71$Ls",
  "3ouoBpTy}7Ggy71$LtX",
  "3ouoBpTy}7Ggy71$LtX{",
  "3ouoBpTy}7Ggy71$LtX{TL",
];

/** `base85::encode` de restes d'octets 0xFF, re-décodés par le crate. Le haut de la plage est
 *  un cas de production réel : `mag_db` va jusqu'à 255 (= 0 dBFS). */
const RUST_HIGH: Array<[string, number[]]> = [
  ["{{", [255]],
  ["|Nj", [255, 255]],
  ["|Ns9", [255, 255, 255]],
  ["|NsC0{{", [255, 255, 255, 255, 255]],
  ["|NsC0|Nj", [255, 255, 255, 255, 255, 255]],
  ["|NsC0|Ns9", [255, 255, 255, 255, 255, 255, 255]],
];

/** Chaînes FORGÉES, hors image de l'encodeur, lues directement dans `base85::decode`.
 *  Chacune est choisie pour que l'octet émis change si la valeur de padding bouge de 1 dans
 *  un sens donné — c'est la seule façon de tenir la constante, voir le test. */
const RUST_PADDING_PROBES: Array<[string, number[]]> = [
  // change si le padding descend (125 suffit)
  ["Ir", [58]],
  ["00!", [0, 7]],
  ["001b", [0, 0, 41]],
  // change si le padding monte (127 suffit)
  ["3i", [10]],
  ["09)", [0, 91]],
  ["001e", [0, 0, 41]],
];

describe("decodeB85 — chemin de reste", () => {
  it("suit le crate sur les longueurs 0 à 17", () => {
    // Un seul `toEqual` sur toute la table plutôt qu'une assertion par tour : l'échec montre
    // alors QUELLE longueur diverge, au lieu de s'arrêter à la première.
    const obtenu: Array<[number, number[]]> = [];
    const attendu: Array<[number, number[]]> = [];
    for (let n = 0; n < RUST_ENCODED.length; n++) {
      obtenu.push([n, Array.from(decodeB85(RUST_ENCODED[n]))]);
      attendu.push([n, SRC.slice(0, n)]);
    }
    expect(obtenu).toEqual(attendu);
  });

  it("rend m octets pour un reste de m+1 caractères", () => {
    // La règle de taille, isolée du contenu. `report-view.ts` lit `mag_db` par `frames * bins` :
    // un octet de trop ou de moins ne lève pas ICI, il décale toute la grille d'une case. C'est
    // `assertSpectrogramLength` qui lève, un cran plus haut.
    expect(decodeB85("3j")).toHaveLength(1);
    expect(decodeB85("3or")).toHaveLength(2);
    expect(decodeB85("3oum")).toHaveLength(3);
    // Et le reste s'ajoute au groupe plein qui le précède, il ne le remplace pas.
    expect(decodeB85("3ouoBp8")).toHaveLength(5);
  });

  it("décode un reste d'octets 0xFF", () => {
    // Ces trois restes portent un accumulateur au-dessus de 2^31 — la zone où l'arithmétique
    // 32 bits de JS cesserait d'être équivalente. Mesuré : `out` étant un `Uint8Array`, le
    // store ramène de toute façon un int32 négatif sur le bon octet, donc ces vecteurs bornent
    // la plage sans épingler l'opérateur. Ils restent la seule couverture du haut de plage.
    const obtenu = RUST_HIGH.map(([enc]) => Array.from(decodeB85(enc)));
    expect(obtenu).toEqual(RUST_HIGH.map(([, bytes]) => bytes));
  });

  it("applique exactement la valeur de padding 126, pas 84", () => {
    // Le commentaire de `b85.ts:63-65` interdit de « corriger » 126 en 84. Aucun vecteur de
    // round-trip ne peut le tenir : mesuré, 84, 85, 125 et 127 redonnent tous les mêmes octets
    // sur les 24 chaînes que l'encodeur produit ci-dessus. La raison est arithmétique — les
    // octets bas de l'entrée sont nuls avant encodage, donc le padding ne fait que remplir un
    // intervalle plus court que le pas de la division finale.
    //
    // D'où les chaînes forgées. L'octet émis est une division entière : une chaîne donnée ne
    // peut trahir le padding que d'UN côté du seuil. Il en faut donc deux par reste, une
    // sensible à 125, une à 127 — et comme la division est monotone, tenir ±1 des deux côtés
    // suffit à exclure toute autre valeur.
    const obtenu = RUST_PADDING_PROBES.map(([enc]) => Array.from(decodeB85(enc)));
    expect(obtenu).toEqual(RUST_PADDING_PROBES.map(([, bytes]) => bytes));
  });

  it("suit le décodeur Rust TEL QU'IL EST EXPÉDIÉ au-delà de u32::MAX", () => {
    // `~` vaut 84, la plus haute valeur de l'alphabet : l'accumulateur monte à 4 437 053 166,
    // au-dessus de u32::MAX. Ces entrées ne peuvent venir que d'une ligne de cache corrompue,
    // l'encodeur ne les produit jamais.
    //
    // Attention à la provenance de ces trois valeurs : `base85::decode` PANIQUE dessus en debug
    // (« attempt to multiply with overflow », `base85-2.0.0/src/lib.rs:158`), donc sous
    // `cargo test`. Elles sont lues sur un build `--release` du crate, où l'arithmétique wrap —
    // `src-tauri/Cargo.toml:89-93` ne surcharge pas `overflow-checks`, donc c'est bien le
    // comportement du binaire distribué. Ce test gèle l'accord avec Sift tel qu'il est livré.
    expect(Array.from(decodeB85("~~"))).toEqual([8]);
    expect(Array.from(decodeB85("~~~"))).toEqual([8, 120]);
    expect(Array.from(decodeB85("~~~~"))).toEqual([8, 120, 14]);
  });
});

// Ce qui reste NON tenu par ces vecteurs, mesuré et non supposé (protocole : rejouer chaque
// vecteur contre une version mutée du décodeur, et regarder laquelle échoue) :
//
// • `% 4294967296` (`b85.ts:72`) — aucune entrée ne peut le tenir, la ligne est arithmétiquement
//   REDONDANTE en JS : chaque octet est déjà réduit par un `% 256` final, et `Math.floor` sur un
//   entier exact commute avec la réduction modulo. Elle documente le wrap u32 du crate, elle ne
//   l'exécute pas. Ne pas la supprimer pour autant — sans elle, le miroir cesse de se lire comme
//   le code Rust qu'il reproduit.
// • `Math.floor(acc / 2^n)` face à `acc >> n` — équivalents ici, parce que `out` est un
//   `Uint8Array` : le store d'un int32 négatif retombe sur le même octet. La forme `Math.floor`
//   reste la bonne (elle est juste indépendamment du type de sortie), mais ce test ne la garde
//   pas — un futur passage à un `number[]` casserait en silence.
//
// Ce que les vecteurs tiennent, en revanche : padding 126 exactement (les deux sens), padding
// absent, garde d'émission `rem > 2` / `rem > 3`, et l'ordre des octets émis.
