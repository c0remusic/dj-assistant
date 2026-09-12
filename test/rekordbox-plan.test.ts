// Plan de synchronisation Rekordbox — une erreur ici est silencieuse et coûteuse : un id de trop
// écrit dans la base d'un autre logiciel, un id de moins laisse un candidat « en attente » sans
// raison. Gelé : la portée (sélection ∩ en attente), la borne de section, et le compte.
import { describe, expect, it } from "vitest";
import { planSync, type SyncPending, type SyncSelection } from "../frontend/rekordbox-plan";

const pending: SyncPending = { repairs: [1, 2], metas: [10], arts: [20, 21], dedups: ["p1::c1"] };
const none: SyncSelection = { repairs: new Set(), metas: new Set(), arts: new Set(), dedups: new Set() };

describe("planSync — tout", () => {
  it("« Tout » prend les quatre tiers, dans l'ordre des tiers", () => {
    const p = planSync("all", pending, none, "all");
    expect(p).toEqual({ repairs: [1, 2], metas: [10], arts: [20, 21], dedups: ["p1::c1"], total: 6 });
  });

  it("une section choisie borne le plan à elle seule", () => {
    expect(planSync("art", pending, none, "all")).toEqual({ repairs: [], metas: [], arts: [20, 21], dedups: [], total: 2 });
    expect(planSync("dedup", pending, none, "all").total).toBe(1);
  });
});

describe("planSync — sélection", () => {
  const sel: SyncSelection = { repairs: new Set([2, 99]), metas: new Set(), arts: new Set([21]), dedups: new Set(["p1::c1"]) };

  it("ne garde que ce qui est coché ET encore en attente", () => {
    // 99 est coché mais n'est plus en attente : il tombe du plan sans bruit.
    expect(planSync("all", pending, sel, "selection")).toEqual({ repairs: [2], metas: [], arts: [21], dedups: ["p1::c1"], total: 3 });
  });

  it("se borne aussi à la section active", () => {
    expect(planSync("files", pending, sel, "selection")).toEqual({ repairs: [2], metas: [], arts: [], dedups: [], total: 1 });
  });

  it("vaut zéro sans rien de coché", () => {
    expect(planSync("all", pending, none, "selection").total).toBe(0);
  });
});
