import { beforeEach, describe, expect, it, vi } from "vitest";

// `library-columns.ts` est de la LOGIQUE PURE au sens de `CLAUDE.md` § Architecture : ordre,
// largeurs, validation de ce qui revient du stockage. Ses deux gestes (glisser un séparateur,
// glisser un en-tête) touchent le DOM et se vérifient dans la vraie fenêtre ; ce qui se teste ici
// est ce dont une erreur serait SILENCIEUSE — une colonne mémorisée qui disparaît, une largeur hors
// bornes, un ordre inconnu adopté tel quel.
//
// Le module lit `localStorage` AU CHARGEMENT, donc chaque cas doit poser son stub avant l'import :
// d'où `vi.resetModules()` + `await import()` plutôt qu'un import statique en tête de fichier.

// `verdict` (colonne 1 de `DESIGN.md` § 16, entrée le 2026-08-19) et `bpm` sont SORTIS le 2026-09-08
// (audit Rangés, #24 : le verdict se lit dans l'inspecteur, BPM n'a jamais été écrit). Ce fichier
// est le seul endroit qui gèle l'ordre : une entrée déplacée ici sans l'être dans `DEFAULT_COLUMNS`
// ne casserait rien à l'écran, juste le contrat que ce test défend.
const FIELDS = ["artist", "title", "duration", "genre", "year"];

function fakeStorage(initial: Record<string, string> = {}) {
  const map = new Map(Object.entries(initial));
  return {
    getItem: (k: string) => map.get(k) ?? null,
    setItem: (k: string, v: string) => void map.set(k, v),
    removeItem: (k: string) => void map.delete(k),
    dump: () => Object.fromEntries(map),
  };
}

async function load(stored?: unknown) {
  const store = fakeStorage(
    stored === undefined ? {} : { "sift-libcols-v1": JSON.stringify(stored) },
  );
  vi.stubGlobal("localStorage", store);
  vi.resetModules();
  const mod = await import("../frontend/library-columns");
  return { mod, store };
}

beforeEach(() => {
  vi.unstubAllGlobals();
});

describe("library-columns — chargement", () => {
  it("rend les cinq colonnes dans l'ordre quand rien n'est mémorisé", async () => {
    const { mod } = await load();
    expect(mod.libraryColumns().map((c) => c.field)).toEqual(FIELDS);
    expect(mod.columnsAreCustomized()).toBe(false);
  });

  it("applique un ordre mémorisé", async () => {
    // Ordre COMPLET (5 champs) : un ordre où il en manque relève du cas « colonne nouvelle »
    // testé plus bas, où la manquante reprend son index par défaut plutôt que la fin.
    const { mod } = await load({
      order: ["duration", "artist", "title", "genre", "year"],
    });
    expect(mod.libraryColumns()[0].field).toBe("duration");
    expect(mod.columnsAreCustomized()).toBe(true);
  });

  // Le cas qui justifie le filtre : une colonne retirée d'une version à l'autre reste dans le
  // stockage de l'utilisateur. L'adopter peindrait une cellule vide par ligne, sans rien dire.
  it("jette une colonne inconnue au lieu de la peindre vide", async () => {
    const { mod } = await load({ order: ["year", "colonne-fantome", "artist"] });
    expect(mod.libraryColumns().map((c) => c.field)).not.toContain("colonne-fantome");
    expect(mod.libraryColumns()).toHaveLength(FIELDS.length);
  });

  // Le cas jumeau : un ordre PARTIEL ne doit pas faire disparaître les colonnes absentes, sinon une
  // donnée quitte l'écran sans que rien ne le signale.
  it("complète un ordre partiel avec les colonnes manquantes", async () => {
    const { mod } = await load({ order: ["genre"] });
    const fields = mod.libraryColumns().map((c) => c.field);
    expect([...fields].sort()).toEqual([...FIELDS].sort());
  });

  // Le cas RÉEL derrière le filtre ET la complétion, dans les deux sens : un stockage écrit entre le
  // 2026-08-19 et le 2026-09-08 porte Verdict et BPM, retirées depuis — elles sont JETÉES, pas
  // peintes vides ; et un stockage d'avant une colonne nouvelle la reçoit à son index par défaut :
  // l'utilisateur n'a jamais exprimé de préférence à son sujet, la disposition mémorisée n'exprime
  // la sienne que sur celles qu'il connaissait.
  it("jette Verdict et BPM d'un stockage d'avant leur retrait, sans toucher au reste", async () => {
    const stale = ["verdict", "artist", "title", "bpm", "duration", "genre", "year"];
    const { mod } = await load({ order: stale, width: { verdict: 141, bpm: 74, genre: 139 } });
    expect(mod.libraryColumns().map((c) => c.field)).toEqual(FIELDS);
    expect(mod.libraryColumns().find((c) => c.field === "genre")?.width).toBe(139);
  });
  it("insère une colonne nouvelle à son index par défaut dans un stockage d'avant elle", async () => {
    const legacy = ["artist", "title", "genre", "year"];
    const { mod } = await load({ order: legacy });
    expect(mod.libraryColumns().map((c) => c.field)).toEqual(["artist", "title", "duration", "genre", "year"]);
  });

  it("ignore un doublon dans l'ordre mémorisé", async () => {
    const { mod } = await load({ order: ["title", "title", "artist"] });
    expect(mod.libraryColumns().filter((c) => c.field === "title")).toHaveLength(1);
  });

  it("survit à un stockage illisible en revenant aux défauts", async () => {
    vi.stubGlobal("localStorage", {
      getItem: () => "{ pas du json",
      setItem: () => undefined,
      removeItem: () => undefined,
    });
    vi.resetModules();
    const mod = await import("../frontend/library-columns");
    expect(mod.libraryColumns().map((c) => c.field)).toEqual(FIELDS);
  });
});

describe("library-columns — largeurs", () => {
  it("borne une largeur mémorisée hors plage", async () => {
    const { mod } = await load({ width: { artist: 5000, genre: 1 } });
    const byField = new Map(mod.libraryColumns().map((c) => [c.field, c.width]));
    expect(byField.get("artist")).toBe(600); // MAX_COL_W
    expect(byField.get("genre")).toBe(48); // MIN_COL_W
  });

  it("borne aussi une largeur posée par le geste, et la persiste", async () => {
    const { mod, store } = await load();
    mod.setColumnWidth("title", 12);
    expect(mod.libraryColumns().find((c) => c.field === "title")?.width).toBe(48);
    expect(JSON.parse(store.dump()["sift-libcols-v1"]).width.title).toBe(48);
  });

  // Sans largeur mémorisée, la colonne DOIT garder sa règle CSS : c'est ce qui lui permet de
  // continuer à s'adapter à la largeur de la zone. Un style inline vide n'est pas un détail.
  it("ne rend aucun style inline tant qu'une colonne n'a pas été redimensionnée", async () => {
    const { mod } = await load();
    expect(mod.columnStyle(mod.libraryColumns()[0])).toBe("");
  });

  it("fige la colonne en px une fois redimensionnée", async () => {
    const { mod } = await load();
    mod.setColumnWidth("duration", 120);
    const col = mod.libraryColumns().find((c) => c.field === "duration");
    expect(col && mod.columnStyle(col)).toBe(' style="flex:none;width:120px"');
  });
});

describe("library-columns — le plancher est partagé avec le CSS", () => {
  // Même patron que `--measure-data` / `MAX_COLS`, épinglé côté Rust par un test qui LIT le CSS :
  // deux fichiers portent le même nombre pour deux travaux différents — le token empêche une
  // colonne VOISINE de s'écraser (c'est le navigateur qui tient l'invariant), la constante borne
  // la colonne qu'on drague. Désaccordés, ils laissent un des deux planchers sans effet, et rien
  // ne le signalerait à l'écran avant qu'une colonne ne devienne illisible.
  it("--col-min-w de styles.css vaut le plancher appliqué par setColumnWidth", async () => {
    const { readFileSync } = await import("node:fs");
    const css = readFileSync(new URL("../frontend/styles.css", import.meta.url), "utf8");
    const m = /--col-min-w:\s*(\d+)px/.exec(css);
    expect(m, "token --col-min-w introuvable dans styles.css").not.toBeNull();

    const { mod } = await load();
    mod.setColumnWidth("artist", 1); // sous n'importe quel plancher plausible
    expect(mod.libraryColumns().find((c) => c.field === "artist")?.width).toBe(Number(m![1]));
  });
});

describe("library-columns — déplacement", () => {
  it("insère avant la colonne visée", async () => {
    const { mod } = await load();
    mod.moveColumn("year", "title");
    expect(mod.libraryColumns().map((c) => c.field)).toEqual(["artist", "year", "title", "duration", "genre"]);
  });

  it("place en fin quand la cible est nulle", async () => {
    const { mod } = await load();
    mod.moveColumn("artist", null);
    expect(mod.libraryColumns().map((c) => c.field)).toEqual(["title", "duration", "genre", "year", "artist"]);
  });

  it("persiste l'ordre et le rétablit à la réinitialisation", async () => {
    const { mod, store } = await load();
    mod.moveColumn("year", "artist");
    // Assertion sur la position relative plutôt que sur l'index 0, qui ne parlait de l'ordre
    // persisté que par la coïncidence d'un défaut.
    const order: string[] = JSON.parse(store.dump()["sift-libcols-v1"]).order;
    expect(order.indexOf("year")).toBeLessThan(order.indexOf("artist"));
    mod.resetColumns();
    expect(mod.libraryColumns().map((c) => c.field)).toEqual(FIELDS);
    expect(store.dump()["sift-libcols-v1"]).toBeUndefined();
    expect(mod.columnsAreCustomized()).toBe(false);
  });
});

describe("library-columns — cohabitation tri / déplacement", () => {
  // Un en-tête est à la fois bouton de tri et poignée de déplacement. Le drapeau est ce qui empêche
  // un réordonnancement de trier en plus, et il se CONSOMME : une seconde lecture doit rendre faux,
  // sinon le clic de tri suivant serait avalé lui aussi.
  it("le drapeau de suppression du tri se consomme une seule fois", async () => {
    const { mod } = await load();
    expect(mod.consumeSortSuppression()).toBe(false);
  });
});
