import { describe, expect, it } from "vitest";
import { estDangereux, modifsSuivies } from "../scripts/check-git-restore.mjs";

// Le détecteur du hook `check-git-restore` (PreToolUse Bash). Vecteurs gelés depuis la ligne
// exacte qui a effacé `worker.rs` le 2026-09-09, et depuis ce qu'un checkout légitime doit
// pouvoir faire. Muter `estDangereux` pour laisser passer `checkout --` fait tomber le premier.

describe("estDangereux", () => {
  it("bloque la ligne réelle du 2026-09-09 (mutation → test → checkout → restauration)", () => {
    const ligne =
      'sed -i "s/a/b/" src-tauri/src/worker.rs && bash scripts/cargo-isolated.sh test --lib -- worker ; ' +
      'git checkout -- src-tauri/src/worker.rs 2>/dev/null; echo "(restauration par sed)"';
    expect(estDangereux(ligne)).toBe(true);
  });

  it.each([
    "git checkout -- frontend/styles.css",
    "git checkout .",
    "git checkout src-tauri/Cargo.toml",
    "git checkout worker.rs",
    "git restore frontend/usb-view.ts",
    "git restore --staged .",
    "git stash",
    "git stash push -m x",
    "cd C:/dev/sift && git -C src-tauri checkout -- src/db.rs",
  ])("bloque %s", (cmd) => {
    expect(estDangereux(cmd)).toBe(true);
  });

  it.each([
    "git checkout main",
    "git checkout -b chantier/usb",
    "git checkout -B main origin/main",
    "git status --short && git log -1",
    "git diff -- src-tauri/src/worker.rs",
    "git show HEAD:frontend/styles.css | head",
    "git commit -F msg.txt && git push origin HEAD:main",
  ])("laisse passer %s", (cmd) => {
    expect(estDangereux(cmd)).toBe(false);
  });
});

describe("modifsSuivies", () => {
  it("ignore les fichiers non suivis : un checkout ne les touche pas", () => {
    const porcelain = ' M src-tauri/Cargo.toml\n?? "docs/design-refs/kit.pdf"\n';
    expect(modifsSuivies(porcelain)).toEqual([" M src-tauri/Cargo.toml"]);
  });

  it("arbre propre = aucune ligne", () => {
    expect(modifsSuivies("")).toEqual([]);
    expect(modifsSuivies("?? nouveau.md\n")).toEqual([]);
  });
});
