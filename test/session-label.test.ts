// Nom d'une session — une erreur ici est silencieuse (un identifiant brut à l'écran, aucun
// crash), périmètre exact de la suite Vitest. Le module est partagé par le Journal et Rekordbox
// depuis le 2026-09-08 : ce test gèle la forme des deux libellés et les deux replis.
import { describe, expect, it } from "vitest";
import { sessionLabel, sessionStart } from "../frontend/session-label";

// Construit en heure LOCALE : `getHours()` lit le fuseau de la machine, un millis fixe donnerait
// une heure différente en CI et sur le poste.
const START = new Date(2026, 8, 8, 1, 24).getTime();
const ID = `${START}-23936`;

describe("sessionLabel", () => {
  it("nomme la session par son heure sous un groupe de jour", () => {
    expect(sessionLabel(ID, false)).toBe("Session de 01h24");
  });

  it("ajoute la date quand la liste n'est pas groupée par jour", () => {
    expect(sessionLabel(ID, true)).toBe("Session du 08/09/2026 01h24");
  });

  it("rend l'identifiant brut plutôt qu'un plantage sur un format inattendu", () => {
    expect(sessionLabel("pas-un-id", true)).toBe("pas-un-id");
    expect(sessionLabel("0-1", false)).toBe("0-1");
  });

  it("dit « Hors session » pour un id absent", () => {
    expect(sessionLabel(null, true)).toBe("Hors session");
  });
});

describe("sessionStart", () => {
  it("lit la partie millis avant le tiret", () => {
    expect(sessionStart(ID)?.getTime()).toBe(START);
    expect(sessionStart("abc-1")).toBeNull();
  });
});
