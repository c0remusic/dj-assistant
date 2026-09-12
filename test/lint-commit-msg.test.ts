// Vecteurs de la gate de message de commit (issue #43). Le script vit dans `scripts/`, tourne sous
// Node et lit un fichier : il s'exécute ici pour de vrai, plutôt que d'être réimplémenté.
//
// Ce que ces cas tiennent, et qu'aucune relecture ne tient : la frontière entre « français
// désaccentué » et « ce qui n'a pas d'accent à porter ». Mesuré sur les 1298 messages de
// l'historique — 218 refusés, dont 13 relus un par un sans un seul faux positif — mais un chiffre
// global ne dit pas OÙ passe la frontière. Ces vecteurs, si.
import { describe, expect, it } from "vitest";
import { execFileSync } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const DIR = mkdtempSync(join(tmpdir(), "sift-msg-"));
let n = 0;

/** Rend le code de sortie du linter sur ce message. 0 = accepté, 1 = refusé. */
function lint(message: string): number {
  const f = join(DIR, `m${n++}.txt`);
  writeFileSync(f, message, "utf8");
  try {
    execFileSync("node", ["scripts/lint-commit-msg.mjs", f], { stdio: "pipe" });
    return 0;
  } catch (e) {
    return (e as { status: number }).status;
  }
}

describe("lint-commit-msg", () => {
  it("refuse une ligne française désaccentuée", () => {
    expect(lint("fix(ui): la fenetre n a aucun rayon, la coque est carree")).toBe(1);
  });

  it("accepte la même ligne accentuée", () => {
    expect(lint("fix(ui): la fenêtre n'a aucun rayon, la coque est carrée")).toBe(0);
  });

  it("accepte du français qui ne demande AUCUN accent", () => {
    // Le mode de défaillance à éviter : une gate qui refuse une phrase correcte se fait désactiver.
    expect(lint("fix(ci): rendre la CI verte et poser la gate qui l'aurait vue")).toBe(0);
    expect(lint("docs: aligner les trois mentions qui restaient dans la spec")).toBe(0);
  });

  it("accepte l'anglais, même quand il contient une forme homographe", () => {
    // `verification`, `execution`, `precedent` sont des mots anglais. Sans le garde de contexte
    // français, tout message anglais du dépôt tomberait.
    expect(lint("fix(rekordbox): write verification failed, restore the backup")).toBe(0);
    expect(lint("refactor: follow the precedent set by the execution path")).toBe(0);
  });

  it("ne compte pas un identifiant de code entre backticks", () => {
    // Faux positif réel, mesuré sur le commit 70f3340 : `echelle` est une variable Rust,
    // `perime.flac` une fixture. Les accentuer casserait le code.
    //
    // ⚠️ La ligne ne porte AUCUN accent, volontairement. La première version de ce vecteur
    // finissait par « gardées » — le linter l'écartait donc sur son accent, pas sur ses backticks,
    // et le cas passait au vert en ne mesurant rien. Attrapé en mutant le script : retirer la
    // neutralisation des backticks ne faisait pas tomber le test.
    expect(lint("fix: la variable `echelle` de `mdct.rs` et la fixture `perime.flac` ne bougent pas")).toBe(0);
  });

  it("ne compte pas un mot CITÉ entre guillemets français", () => {
    expect(lint("docs: les 18 occurrences de « verification » dans ce fichier sont de l'anglais")).toBe(0);
  });

  it("compte une CITATION DE PROSE de plus de deux mots, elle", () => {
    // La citation courte désigne le mot ; au-delà, c'est de la prose, et la règle s'applique.
    expect(lint("docs: Antoine dit « la fenetre est carree et sans rayon » dans sa note")).toBe(1);
  });

  it("lit le corps et pas seulement le titre", () => {
    expect(lint("feat(revue): un titre parfaitement accentué\n\nMais le corps parle du systeme et de sa regle deja posee.")).toBe(1);
  });

  it("ignore les lignes de commentaire de git", () => {
    // `git commit -v` colle le diff entier derrière des `#` : le lire ferait échouer sur du code
    // que ce commit ne fait que déplacer.
    expect(lint("feat: un titre accentué correct\n\n# Please enter the commit message\n# la regle du systeme deja posee\n")).toBe(0);
  });
});
