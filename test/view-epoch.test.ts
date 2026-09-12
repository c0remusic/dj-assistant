// Jeton de génération de rendu (issue #42) — logique pure, sans DOM : périmètre exact de la suite
// Vitest en env Node. Une erreur ici est SILENCIEUSE à l'exécution : un jeton qui ne se périme pas
// ne jette rien, il laisse simplement un rendu en retard repeindre l'écran courant — exactement le
// bug qu'on corrige, et qui ne se voit qu'en fenêtre réelle sous scan.
//
// Le module porte un compteur de MODULE, jamais remis à zéro (c'est le contrat : un jeton périmé
// doit le rester pour toute la session). Les tests sont donc écrits en RELATIF — on capture, on
// bouscule, on compare — plutôt qu'en absolu sur une valeur de départ, et aucun `beforeEach` de
// remise à zéro n'est nécessaire ni souhaitable.
import { describe, expect, it } from "vitest";
import { bumpViewEpoch, isStaleViewRender, viewEpoch } from "../frontend/view-epoch";

describe("view-epoch — fraîcheur d'un rendu", () => {
  it("un jeton capturé sans navigation entre-temps reste valide", () => {
    const token = viewEpoch();
    expect(isStaleViewRender(token)).toBe(false);
  });

  it("une navigation périme le jeton capturé avant elle — le cœur de l'issue #42", () => {
    const token = viewEpoch(); // le renderer de l'écran A démarre
    bumpViewEpoch(); // l'utilisateur passe sur l'écran B pendant l'aller-retour IPC
    expect(isStaleViewRender(token)).toBe(true);
  });

  it("le jeton ouvert par la navigation est, lui, le jeton courant", () => {
    const fresh = bumpViewEpoch();
    expect(fresh).toBe(viewEpoch());
    expect(isStaleViewRender(fresh)).toBe(false);
  });

  it("DEUX rendus concurrents SANS navigation partagent le jeton — limite assumée du garde", () => {
    // Le jeton distingue les générations, pas les appels : deux `renderBiblioLive()` lancés par
    // deux frappes de recherche successives portent le même jeton et écriront tous les deux
    // (dernier arrivé, dernier peint). C'est une course intra-écran, hors périmètre — gelée ici
    // pour que personne ne croie que ce module la couvre.
    const a = viewEpoch();
    const b = viewEpoch();
    expect(a).toBe(b);
    expect(isStaleViewRender(a)).toBe(false);
    expect(isStaleViewRender(b)).toBe(false);
  });

  it("le compteur est monotone croissant : un jeton périmé ne redevient JAMAIS valide", () => {
    // Le recyclage serait le pire mode d'échec possible — un rendu très en retard redeviendrait
    // légitime après un tour de compteur, et repeindrait un écran au hasard.
    const token = viewEpoch();
    const seen = new Set<number>([token]);
    for (let i = 0; i < 200; i++) {
      const next = bumpViewEpoch();
      expect(seen.has(next)).toBe(false);
      expect(next).toBeGreaterThan(token);
      seen.add(next);
      expect(isStaleViewRender(token)).toBe(true);
    }
  });

  it("bumpViewEpoch avance d'exactement un cran", () => {
    const before = viewEpoch();
    expect(bumpViewEpoch()).toBe(before + 1);
    expect(viewEpoch()).toBe(before + 1);
  });
});
