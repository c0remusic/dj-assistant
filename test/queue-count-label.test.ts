import { describe, expect, it } from "vitest";
import { queueCountLabel, type QueueCountInput } from "../frontend/queue-count-label";

// Gate de cran 1 pour le compte de la barre unifiée de Revue (issue #49). Ce que ces vecteurs
// tiennent, et qu'aucune autre gate ne tient : le compte change de POPULATION selon l'état — file
// entière au repos, liste visible sous filtre, sélection en mode Lot. Une régression y est
// silencieuse (un nombre plausible s'affiche quand même), d'où le gel.
const base: QueueCountInput = { mode: "detail", selected: 0, total: 3124, visible: 3124, filtered: false };

describe("queueCountLabel", () => {
  it("dit la file ENTIÈRE quand rien n'est posé", () => {
    expect(queueCountLabel(base)).toBe("3124 pistes");
  });

  it("dit ce que la colonne MONTRE quand un filtre est posé", () => {
    expect(queueCountLabel({ ...base, visible: 139, filtered: true })).toBe("139 pistes filtrées");
  });

  it("ne lit JAMAIS le total sous filtre — c'est la perte que #49 corrige", () => {
    // Le piège exact d'avant le correctif : `visible` ignoré, `total` affiché. Si cette assertion
    // tombe, le compte est redevenu celui de la file entière sous un filtre actif.
    const out = queueCountLabel({ ...base, visible: 139, filtered: true });
    expect(out).not.toContain("3124");
  });

  it("garde le mot au singulier à 0 et à 1, au pluriel au-delà", () => {
    expect(queueCountLabel({ ...base, total: 0, visible: 0 })).toBe("0 piste");
    expect(queueCountLabel({ ...base, total: 1, visible: 1 })).toBe("1 piste");
    expect(queueCountLabel({ ...base, total: 2, visible: 2 })).toBe("2 pistes");
    // Le participe s'accorde avec le nom : les deux marques bougent ensemble, jamais l'une seule.
    expect(queueCountLabel({ ...base, visible: 0, filtered: true })).toBe("0 piste filtrée");
    expect(queueCountLabel({ ...base, visible: 1, filtered: true })).toBe("1 piste filtrée");
    expect(queueCountLabel({ ...base, visible: 2, filtered: true })).toBe("2 pistes filtrées");
  });

  it("dit la SÉLECTION en mode Lot, et elle remplace le total", () => {
    expect(queueCountLabel({ ...base, mode: "batch", selected: 5 })).toBe("5 sélectionnées");
    expect(queueCountLabel({ ...base, mode: "batch", selected: 1 })).toBe("1 sélectionnée");
    expect(queueCountLabel({ ...base, mode: "batch", selected: 0 })).toBe("0 sélectionnée");
  });

  it("laisse le mode Lot passer AVANT le filtre — la sélection est ce que le lot traitera", () => {
    const out = queueCountLabel({ ...base, mode: "batch", selected: 5, visible: 139, filtered: true });
    expect(out).toBe("5 sélectionnées");
  });
});
