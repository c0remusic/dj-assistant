// Teinte d'identité des sources du rail — une erreur ici est silencieuse (mauvaise couleur,
// aucun crash), périmètre exact de la suite Vitest. Trois choses gelées :
// 1. l'ordre du cycle (il fixe quelle teinte reçoit chaque position d'ajout) ;
// 2. la position = ordre d'id croissant, PAS l'ordre d'affichage de la liste passée ;
// 3. le miroir TS ↔ CSS : chaque teinte du cycle a sa classe `.sift-rail-src-dot-<teinte>`
//    dans styles.css — c'est ce qui a cassé en silence quand 57f64b2 a renommé la famille,
//    et ce qui casserait pareil au prochain renommage.
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { resolveSourceColorKey, SOURCE_HUE_CYCLE } from "../frontend/source-color";
import type { Source } from "../shared/contracts";

function src(id: number, color_key: string | null = null): Source {
  return { id, path: `C:\\music\\${id}`, pending_count: 0, track_count: 12, accessible: true, watched: true, color_key };
}

describe("SOURCE_HUE_CYCLE", () => {
  it("garde l'ordre gelé — il détermine la teinte de chaque position d'ajout", () => {
    expect(SOURCE_HUE_CYCLE).toEqual(["indigo", "purple", "pink", "teal", "yellow"]);
  });

  it("a sa classe .sift-rail-src-dot-<teinte> dans styles.css pour chaque teinte", () => {
    const css = readFileSync(new URL("../frontend/styles.css", import.meta.url), "utf8");
    for (const hue of SOURCE_HUE_CYCLE) {
      expect(css, `classe manquante pour « ${hue} »`).toContain(`.sift-rail-src-dot-${hue}{`);
    }
  });
});

describe("resolveSourceColorKey", () => {
  it("cycle par ordre d'ajout (id croissant), indépendant de l'ordre d'affichage", () => {
    // Liste volontairement mélangée : seule la position par id doit compter.
    const sources = [src(30), src(10), src(50), src(20), src(60), src(40)];
    const byId = new Map(sources.map((s) => [s.id, resolveSourceColorKey(sources, s)]));
    expect(byId.get(10)).toBe("indigo");
    expect(byId.get(20)).toBe("purple");
    expect(byId.get(30)).toBe("pink");
    expect(byId.get(40)).toBe("teal");
    expect(byId.get(50)).toBe("yellow");
    expect(byId.get(60)).toBe("indigo"); // 6e source : le cycle reboucle
  });

  it("laisse l'override manuel primer sur la position", () => {
    const sources = [src(10), src(20), src(30, "teal")];
    // Par position, 30 serait « pink » — l'override doit gagner.
    expect(resolveSourceColorKey(sources, sources[2])).toBe("teal");
    // Et il ne décale pas les voisines : elles gardent leur position d'ajout.
    expect(resolveSourceColorKey(sources, sources[0])).toBe("indigo");
    expect(resolveSourceColorKey(sources, sources[1])).toBe("purple");
  });
});
