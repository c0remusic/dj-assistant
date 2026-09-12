// Un chemin absolu vers l'arborescence d'une machine de développement n'existe sur aucune
// autre : selon le chemin de code, il produit une erreur peu claire ou un comportement
// silencieusement faux. Il publie aussi le nom du compte utilisateur. Issue #4.
//
// La « définition de fait » du ticket demandait exactement ceci : « un balayage du dépôt sur
// le motif de chemin concerné, AVEC UN COMPTE DE FICHIERS BALAYÉS NON NUL, ne rend plus aucun
// résultat dans le code d'application ». Ce balayage a été fait une fois, à la fermeture. Rien
// ne le rejouait. Le voici récurrent, témoin compris — un balayage vide et un balayage propre
// se ressemblent exactement, c'est le mode de panne de #29 sur `spacingDeclsSeen`.
//
// PÉRIMÈTRE : code d'application seulement. Le ticket note lui-même que le même motif dans la
// documentation est moins grave, « pas d'effet à l'exécution », et se traite séparément.
import { describe, expect, it } from "vitest";
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, extname } from "node:path";

const RACINES = ["frontend", "shared", "scripts", "src-tauri/src"];
const EXTENSIONS = new Set([".ts", ".js", ".mjs", ".cjs", ".rs"]);

// Dossier utilisateur nommé, ou arborescence de travail à lettre fixe. Volontairement PAS
// « toute lettre de lecteur » : `usb_format/` et `ipc_usage.rs` manipulent légitimement des
// racines de volume (`"I:"`, `r"\\.\I:"`) et un motif large les prendrait, se ferait désactiver,
// et ne garderait plus rien.
const MOTIFS: Array<[string, RegExp]> = [
  ["dossier utilisateur Windows", /[A-Za-z]:[\\/]{1,2}Users[\\/]{1,2}[A-Za-z0-9._-]+/i],
  ["dossier utilisateur macOS", /\/Users\/[A-Za-z0-9._-]+\//],
  ["dossier utilisateur Linux", /\/home\/[A-Za-z0-9._-]+\//],
  ["arborescence de travail", /[A-Za-z]:[\\/]{1,2}dev[\\/]{1,2}/i],
];

interface Trouvaille {
  fichier: string;
  ligne: number;
  motif: string;
  extrait: string;
}

function balayer(): { trouvailles: Trouvaille[]; fichiersBalayes: number } {
  const trouvailles: Trouvaille[] = [];
  let fichiersBalayes = 0;

  const descendre = (dir: string): void => {
    let entrees: string[];
    try {
      entrees = readdirSync(dir);
    } catch {
      return; // racine absente dans ce checkout — le témoin le rendra visible
    }
    for (const nom of entrees) {
      if (nom === "node_modules" || nom === "target" || nom === "dist") continue;
      const p = join(dir, nom);
      if (statSync(p).isDirectory()) {
        descendre(p);
        continue;
      }
      if (!EXTENSIONS.has(extname(nom))) continue;

      fichiersBalayes++;
      let lignes = readFileSync(p, "utf8").split(/\r?\n/);

      // Le ticket vise « le code d'application », explicitement « des valeurs utilisées à
      // l'exécution ». Une fixture de test n'en est pas une. Convention du dépôt, énoncée par
      // `.claude/rules/rust.md` : « Tests : `#[cfg(test)] mod tests` inline par module » — donc
      // en fin de fichier. Couper au premier `#[cfg(test)]` est exact ici, et se verrait tomber
      // si la convention changeait, parce que le témoin de fichiers balayés ne bougerait pas
      // pendant que des chemins de test réapparaîtraient.
      if (extname(nom) === ".rs") {
        const iTest = lignes.findIndex((l) => l.trim().startsWith("#[cfg(test)]"));
        if (iTest !== -1) lignes = lignes.slice(0, iTest);
      }

      lignes.forEach((ligne, i) => {
        // Une ligne de commentaire n'a pas d'effet à l'exécution — c'est la distinction que
        // le ticket pose lui-même entre code et documentation.
        const nu = ligne.trim();
        if (nu.startsWith("//") || nu.startsWith("*") || nu.startsWith("/*") || nu.startsWith("#"))
          return;
        for (const [motif, re] of MOTIFS) {
          const m = ligne.match(re);
          if (m) trouvailles.push({ fichier: p, ligne: i + 1, motif, extrait: m[0] });
        }
      });
    }
  };

  for (const r of RACINES) descendre(r);
  return { trouvailles, fichiersBalayes };
}

describe("chemins de machine dans le code d'application (issue #4)", () => {
  it("n'en contient aucun", () => {
    const { trouvailles } = balayer();
    expect(
      trouvailles.map((t) => `${t.fichier}:${t.ligne} [${t.motif}] ${t.extrait}`),
    ).toEqual([]);
  });

  it("a réellement balayé quelque chose", () => {
    // Le compte non nul que la définition de fait du ticket exigeait. Sans lui, une racine
    // renommée rendrait ce test vert en ne lisant plus rien.
    expect(balayer().fichiersBalayes).toBeGreaterThanOrEqual(50);
  });
});
