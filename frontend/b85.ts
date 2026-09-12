// RFC1924 base85 decoder — exact mirror of the `base85` 2.0.0 crate used on the Rust side
// (src-tauri/src/b85_bytes.rs, `#[serde(with = "crate::b85_bytes")]`). Kept hand-written on
// purpose: adding a frontend dependency needs human sign-off in this repo, and the algorithm is
// 40 lines.
//
// Why it exists: `serde_json` writes a `Vec<u8>` as `[0,12,255,…]` (~3.7 chars/byte). The
// analysis report's spectrogram (`mag_db`, ~360k bytes per track) now travels as base85
// (1.25 chars/byte) instead. Only the ENCODING changed — the payload is still frames*bins
// bytes, row-major, 0 = -100 dBFS, 255 = 0 dBFS.
//
// The alphabet and the remainder rule below are copied literally from the crate
// (base85-2.0.0/src/lib.rs:31-33 for the table, :125-176 for `decode`). Do not "fix" the 126
// padding value: it is what the encoder's inverse expects, and the Rust round-trip test in
// b85_bytes.rs pins it.

/** base85 value -> character, index = value. Contains neither `"` nor `\`, so a string encoded
 *  with it never needs escaping inside JSON. Literal copy of base85-2.0.0/src/lib.rs:31-33. */
const B85_ALPHABET =
  "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz!#$%&()*+-;<=>?@^_`{|}~";

/** Reverse table, character code -> base85 value, -1 for anything outside the alphabet. */
const B85_VALUES: Int16Array = (() => {
  const t = new Int16Array(128).fill(-1);
  for (let i = 0; i < B85_ALPHABET.length; i++) t[B85_ALPHABET.charCodeAt(i)] = i;
  return t;
})();

function valueAt(s: string, i: number): number {
  const code = s.charCodeAt(i);
  const v = code < 128 ? B85_VALUES[code] : -1;
  // Loud failure on purpose. Returning 0 here would paint an all-black spectrogram with no
  // error anywhere — the exact silent failure mode this decoder exists to avoid.
  if (v < 0) throw new Error(`base85 decode: invalid character ${JSON.stringify(s[i])} at ${i}`);
  return v;
}

/** Decodes an RFC1924 base85 string into its bytes. `""` -> empty array (the backend's "no
 *  spectrogram" sentinel, see analysis/spectrum.rs). Throws on any character outside the
 *  alphabet or on a trailing group of a single character (the crate's UnexpectedEof). */
export function decodeB85(s: string): Uint8Array {
  const full = Math.floor(s.length / 5);
  const rem = s.length - full * 5;
  if (rem === 1) throw new Error("base85 decode: unexpected end of input");
  const out = new Uint8Array(full * 4 + (rem > 0 ? rem - 1 : 0));

  let o = 0;
  for (let g = 0; g < full; g++) {
    const i = g * 5;
    const acc =
      valueAt(s, i) * 52200625 + // 85^4
      valueAt(s, i + 1) * 614125 + // 85^3
      valueAt(s, i + 2) * 7225 + // 85^2
      valueAt(s, i + 3) * 85 +
      valueAt(s, i + 4);
    out[o++] = Math.floor(acc / 16777216) % 256;
    out[o++] = Math.floor(acc / 65536) % 256;
    out[o++] = Math.floor(acc / 256) % 256;
    out[o++] = acc % 256;
  }

  if (rem > 0) {
    const i = full * 5;
    // Missing digits are padded with the VALUE 126 (not 84), matching the crate's
    // `map_or(Ok(126), char85_to_byte)` at base85-2.0.0/src/lib.rs:158-162. `% 4294967296`
    // reproduces the u32 wrap of that same arithmetic.
    const acc =
      (valueAt(s, i) * 52200625 +
        valueAt(s, i + 1) * 614125 +
        (rem > 2 ? valueAt(s, i + 2) : 126) * 7225 +
        (rem > 3 ? valueAt(s, i + 3) : 126) * 85 +
        126) %
      4294967296;
    out[o++] = Math.floor(acc / 16777216) % 256;
    if (rem > 2) out[o++] = Math.floor(acc / 65536) % 256;
    if (rem > 3) out[o++] = Math.floor(acc / 256) % 256;
  }

  return out;
}
