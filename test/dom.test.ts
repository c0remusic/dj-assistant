import { describe, expect, it } from "vitest";
import { esc } from "../frontend/dom";

// `esc()` est la seule barrière entre des données non maîtrisées (noms de fichiers, tags
// ID3, champs Discogs, lignes de `master.db`) et des dizaines de `innerHTML =` dans le
// frontend. CLAUDE.md § Front enregistre qu'un XSS stocké RÉEL a été livré par le seul
// fichier qui avait oublié de l'appeler (`journal.ts`, audit du 2026-07-10).
//
// Note d'import : `frontend/dom.ts` exporte aussi `requireEl`, qui touche `document`.
// L'import reste sûr en environnement Node parce que `document` y est un paramètre par
// défaut, évalué à l'APPEL et non au chargement du module.

describe("esc", () => {
  it("échappe les cinq caractères qui ouvrent une balise ou un attribut", () => {
    expect(esc("&")).toBe("&amp;");
    expect(esc("<")).toBe("&lt;");
    expect(esc(">")).toBe("&gt;");
    expect(esc('"')).toBe("&quot;");
    expect(esc("'")).toBe("&#39;");
  });

  it("neutralise une charge utile de balise dans un nom de fichier", () => {
    expect(esc("<script>alert(1)</script>.mp3")).toBe(
      "&lt;script&gt;alert(1)&lt;/script&gt;.mp3",
    );
  });

  it("n'est PAS idempotent — donc il s'applique exactement une fois par site", () => {
    // Gelé volontairement. Un double appel produit du texte visiblement abîmé (`&amp;amp;`)
    // au lieu d'une faille : le mode d'échec d'un `esc()` en trop est cosmétique et se voit,
    // celui d'un `esc()` en moins est un XSS et ne se voit pas. Si quelqu'un rend `esc()`
    // idempotent pour « réparer » l'affichage, il rend aussi le double échappement
    // indétectable — ce test doit échouer avant.
    expect(esc(esc("&"))).toBe("&amp;amp;");
  });

  // ────────────────────────────────────────────────────────────────────────────────────
  // LA FRONTIÈRE DE `esc()`, ACTÉE. Décision du 2026-08-05 : `esc()` couvre le TEXTE et
  // les valeurs d'attribut ENTRE GUILLEMETS, et rien d'autre. Ce n'est pas un aveu de
  // faiblesse mais le contrat exact dont Sift a besoin — l'audit des sites d'appel le
  // montre, et c'est cet audit qui doit être refait avant de toucher aux trois tests
  // ci-dessous, pas leur seule lecture.
  //
  // Ce qui a été vérifié :
  //   1. AUCUNE interpolation dans un `href`. Le seul `<a>` du frontend
  //      (`reglages-view.ts:83`) n'a même pas d'attribut `href` — la navigation passe par
  //      `openUrl()`.
  //   2. Une URL ne devient donc JAMAIS un attribut : elle part en IPC vers Rust, où
  //      `open_url` (`src-tauri/src/ipc.rs:485`) refuse tout ce qui n'est pas `http://`
  //      ou `https://`. La barrière des schémas d'URL est là-bas, pas ici.
  //   3. Aucun attribut NON quoté construit depuis des données.
  //   4. Le seul `src` réellement externe est celui d'un `<img>` alimenté par Discogs
  //      (`identify-shared.ts:12`) — et ni `javascript:` ni `data:` ne s'exécutent dans
  //      un `<img src>`. Ce qu'il faut y empêcher est la SORTIE des guillemets, ce que
  //      `esc()` fait.
  //
  // Ce qui rouvrirait la question, et rendrait ces trois tests faux : le premier
  // `href="${…}"`, le premier attribut interpolé sans guillemets, ou la première donnée
  // posée dans un `<script>`/`<style>`. Dans ces cas il faut une SECONDE fonction
  // (`safeUrl` / `escAttr`), pas un `esc()` élargi — élargir `esc()` alourdirait les
  // dizaines de sites qui n'en ont pas besoin.
  // ────────────────────────────────────────────────────────────────────────────────────

  it("laisse passer une URL `javascript:` — hors de son contrat, et sans site d'appel", () => {
    // Aucun des cinq caractères n'apparaît dans cette charge : `esc()` la rend intacte.
    // Inoffensif ici uniquement parce qu'aucun `href` du frontend n'est construit depuis
    // des données (point 1 ci-dessus) et que Rust filtre les schémas (point 2).
    expect(esc("javascript:alert(1)")).toBe("javascript:alert(1)");
  });

  it("ne protège pas un attribut non quoté — aucun n'est construit depuis des données", () => {
    // `<span class=${esc(v)}>` sortirait de la valeur sur le simple espace, sans qu'aucun
    // caractère échappable soit en jeu. Gelé pour que la limite soit lisible dans le test
    // plutôt que déduite du code de `esc()`.
    expect(esc("a onerror=alert(1)")).toBe("a onerror=alert(1)");
  });

  it("enferme une charge Discogs dans les guillemets d'un attribut", () => {
    // Le cas réel : `<img src="${esc(c.cover_url)}">`, `cover_url` venant de l'API Discogs.
    // Le guillemet fermant devient `&quot;`, donc la charge reste UNE valeur d'attribut au
    // lieu de devenir un `onerror`. C'est la propriété qui compte sur ce site d'appel.
    expect(esc('x" onerror="alert(1)')).toBe("x&quot; onerror=&quot;alert(1)");
  });
});
