// Vecteurs du hook PreToolUse qui garde les titres `gh` (issue #43, dernier canal du strip).
// Deux niveaux : l'extraction (import direct), et le hook ENTIER via stdin JSON — c'est le
// contrat réel de Claude Code, et c'est lui qui doit bloquer (exit 2) ou laisser passer (0).
import { describe, expect, it } from "vitest";
import { spawnSync } from "node:child_process";
import { extraireTitres } from "../scripts/check-gh-title.mjs";

function hook(toolName: string, command: string): { code: number | null; stderr: string } {
  const r = spawnSync(process.execPath, ["scripts/check-gh-title.mjs"], {
    input: JSON.stringify({ tool_name: toolName, tool_input: { command } }),
    encoding: "utf8",
  });
  return { code: r.status, stderr: r.stderr };
}

describe("extraireTitres", () => {
  it("lit les guillemets doubles, simples, et la forme --title=", () => {
    expect(extraireTitres('gh issue create --title "Un titre" --body-file f.md')).toEqual(["Un titre"]);
    expect(extraireTitres("gh issue edit 12 --title 'Autre titre'")).toEqual(["Autre titre"]);
    expect(extraireTitres('gh pr create --title="Collé au égal"')).toEqual(["Collé au égal"]);
  });

  it("ne prend pas un -t collé à un autre mot, et rend vide sans titre", () => {
    expect(extraireTitres("gh issue list --state open")).toEqual([]);
    expect(extraireTitres("sort -t: fichier")).toEqual([]);
  });
});

describe("hook complet (stdin JSON)", () => {
  it("BLOQUE (exit 2) un titre désaccentué — le bug exact de #38-#41", () => {
    const r = hook("Bash", 'gh issue create --title "La fenetre n a aucun rayon, la coque est carree"');
    expect(r.code).toBe(2);
    expect(r.stderr).toContain("désaccentué");
  });

  it("laisse passer un titre accentué", () => {
    expect(hook("Bash", 'gh issue edit 41 --title "La fenêtre n\'a aucun rayon de coin"').code).toBe(0);
  });

  it("laisse passer l'anglais et les commandes gh sans titre", () => {
    expect(hook("Bash", 'gh pr create --title "Fix the release verification path"').code).toBe(0);
    expect(hook("Bash", "gh run list --limit 3").code).toBe(0);
  });

  it("ignore les autres outils et le JSON invalide — fail-open", () => {
    // ⚠️ Le titre DOIT être un strip que le linter refuserait (vérifié par le cas « BLOQUE » plus
    // haut, même phrase) : c'est ce qui prouve que l'exit 0 vient du filtre tool_name, pas d'un
    // titre trop court pour le garde de contexte. Première version : « regle deja fausse », un
    // seul mot-outil — la mutation « filtre retiré » passait au vert sans rien mesurer.
    expect(hook("Edit", 'gh issue create --title "La fenetre n a aucun rayon, la coque est carree"').code).toBe(0);
    const r = spawnSync(process.execPath, ["scripts/check-gh-title.mjs"], { input: "pas du json", encoding: "utf8" });
    expect(r.status).toBe(0);
  });
});
