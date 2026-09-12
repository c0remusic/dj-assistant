// Genre → family resolution for chip coloring (2026-07-06 Apple system-colors
// palette). Frontend-only concern — genres.rs stays a plain free-form string
// store, no DB/backend change. Matching is case-insensitive substring search,
// not exact-match: real Discogs "style" strings vary in formulation ("Deep
// House", "House", "Tech House" all need to resolve to the same family).
export type GenreFamily = "house" | "techno" | "discofunksoul" | "hiphop" | "autre";

interface FamilyDef {
  family: GenreFamily;
  keywords: string[];
}

// Order matters: first matching keyword wins. Keep specific-before-generic
// if a future keyword could overlap two families.
const FAMILIES: FamilyDef[] = [
  { family: "house", keywords: ["house", "garage"] },
  { family: "techno", keywords: ["techno", "electro", "industrial", "ebm", "trance", "acid", "minimal", "goa"] },
  { family: "discofunksoul", keywords: ["disco", "funk", "soul", "boogie"] },
  { family: "hiphop", keywords: ["hip hop", "hip-hop", "rap", "r&b", "rnb", "trap"] },
];

/** Resolves a raw Discogs genre string to a coloring family. Unrecognized
 *  genres (including empty strings) fall back to "autre" (neutral, no color). */
export function resolveGenreFamily(genre: string): GenreFamily {
  const norm = genre.trim().toLowerCase();
  if (!norm) return "autre";
  for (const def of FAMILIES) {
    if (def.keywords.some((kw) => norm.includes(kw))) return def.family;
  }
  return "autre";
}
