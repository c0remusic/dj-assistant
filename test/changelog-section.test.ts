// Le pied de page des notes de release est le SEUL texte que lit quelqu'un à qui Antoine envoie
// un lien. Il part aussi dans le champ `notes` de `latest.json`, que chaque installation existante
// télécharge — et qu'éditer la page GitHub après coup ne change pas.
//
// Ce qu'on garde ici et pourquoi : le 2026-08-16, la première installation sans accompagnement a
// échoué parce que la personne a téléchargé `Sift_0.0.3_aarch64.app.tar.gz` (l'artefact
// d'auto-update, 34 198 810 octets) au lieu du `.dmg` (33 204 781). Le fichier de mise à jour pèse
// PLUS LOURD que l'installeur, donc il a l'air d'être le bon. La doc était juste ; c'est la
// désignation du fichier qui manquait.
//
// Le test lance le VRAI script sur le VRAI `CHANGELOG.md`, pas une copie de son texte : une
// constante recopiée ici ne tomberait pas si quelqu'un la retirait du script.
import { describe, expect, it } from "vitest";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";

/** Première version réellement présente dans CHANGELOG.md — le test suit le fichier plutôt que
 *  d'épingler un numéro qui périmerait à la release suivante. */
function premiereVersion(): string {
  const md = readFileSync("CHANGELOG.md", "utf8");
  const m = md.match(/^## (v\d+\.\d+\.\d+)\s*$/m);
  if (!m) throw new Error("CHANGELOG.md n'a aucune section de version");
  return m[1];
}

function notes(tag: string): string {
  return execFileSync("node", ["scripts/changelog-section.mjs", tag], {
    encoding: "utf8",
  });
}

describe("notes de release", () => {
  it("nomme un seul fichier à télécharger par machine", () => {
    const sortie = notes(premiereVersion());
    expect(sortie).toContain("x64-setup.exe");
    expect(sortie).toContain("aarch64.dmg");
  });

  it("écarte nommément les artefacts qui ne s'installent pas", () => {
    const sortie = notes(premiereVersion());
    // `.app.tar.gz` est celui qui a réellement été téléchargé par erreur : il doit être nommé,
    // pas seulement sous-entendu par « prendre le .dmg ».
    expect(sortie).toContain(".app.tar.gz");
    expect(sortie).toContain("ne s'installe pas");
  });

  it("donne les deux messages de Gatekeeper, dont celui qui n'offre aucun bouton", () => {
    const sortie = notes(premiereVersion());
    expect(sortie).toContain("développeur non identifié");
    expect(sortie).toContain("is damaged");
    // Le `-r` est la partie qui compte : un `.app` est un dossier, `xattr -d` seul ne suffit pas.
    expect(sortie).toContain("xattr -dr com.apple.quarantine");
  });

  it("mène au manuel, qui est le seul texte disant comment s'en servir", () => {
    // Installer et utiliser ne sont pas le même moment (issue #19). Quelqu'un qui reçoit le lien
    // n'ouvre pas forcément le dépôt : sans cette ligne, le manuel n'existe que pour qui le fait.
    expect(notes(premiereVersion())).toContain("docs/manuel.md");
    expect(notes(premiereVersion())).toContain(`/releases/download/${premiereVersion()}/manuel.pdf`);
  });

  it("contient les notes de la version, pas seulement le pied de page", () => {
    const tag = premiereVersion();
    const sortie = notes(tag);
    const md = readFileSync("CHANGELOG.md", "utf8");
    const debut = md.indexOf(`## ${tag}`);
    const premiereLigne = md
      .slice(debut)
      .split(/\r?\n/)
      .slice(1)
      .find((l) => l.trim().length > 0);
    expect(premiereLigne).toBeTruthy();
    expect(sortie).toContain(premiereLigne!.trim());
  });

  it("échoue au lieu de publier des notes vides sur une version absente", () => {
    // Le fail-fast est le comportement critique : une section manquante doit faire ÉCHOUER la
    // release, sinon chaque installation existante reçoit un `notes` vide sans que rien ne le dise.
    expect(() => notes("v99.99.99")).toThrow();
  });
});
