// Ligne de source du rail — une erreur ici est silencieuse (mauvaise classe d'état, aucune
// exception), périmètre exact de la suite Vitest. Deux choses gelées :
// 1. la PRÉCÉDENCE des états (rail.md § États) : l'échec prime sur la suspension — « jamais
//    atténuée » — et la suspension ne se rend que sans échec ;
// 2. l'échappement : chemin et motif d'échec traversent `esc()` avant l'attribut `title`.
import { describe, expect, it } from "vitest";
import { baseName, railRowState, railShapeKey, sourceEntryHtml } from "../frontend/rail-source-entry";
import type { Source } from "../shared/contracts";

function src(over: Partial<Source> = {}): Source {
  return {
    id: 1,
    path: "C:\\music\\incoming",
    pending_count: 0,
    track_count: 12,
    accessible: true,
    watched: true,
    color_key: null,
    ...over,
  };
}

describe("sourceEntryHtml — états", () => {
  it("source saine et surveillée : ni --error ni --suspended", () => {
    const html = sourceEntryHtml(src(), [src()], false, undefined);
    expect(html).not.toContain("sift-rail-src--error");
    expect(html).not.toContain("sift-rail-src--suspended");
  });

  it("surveillance suspendue : classe --suspended et motif dans le title", () => {
    const s = src({ watched: false });
    const html = sourceEntryHtml(s, [s], false, undefined);
    expect(html).toContain("sift-rail-src--suspended");
    expect(html).toContain("surveillance suspendue");
  });

  it("l'échec de scan PRIME sur la suspension — jamais atténuée (rail.md § États)", () => {
    const s = src({ watched: false });
    const html = sourceEntryHtml(s, [s], false, "verrou");
    expect(html).toContain("sift-rail-src--error");
    expect(html).not.toContain("sift-rail-src--suspended");
    expect(html).toContain("scan en échec");
  });

  it("le dossier inaccessible PRIME sur la suspension, même sans échec de scan", () => {
    const s = src({ watched: false, accessible: false });
    const html = sourceEntryHtml(s, [s], false, undefined);
    expect(html).toContain("sift-rail-src--error");
    expect(html).not.toContain("sift-rail-src--suspended");
    expect(html).toContain("dossier inaccessible");
  });

  it("suspendue garde sa teinte : la pastille identifie, l'état ne la vide que par CSS", () => {
    const s = src({ watched: false });
    const html = sourceEntryHtml(s, [s], false, undefined);
    expect(html).toMatch(/sift-rail-src-dot-(indigo|purple|pink|teal|yellow)/);
  });

  // Issue #55, décision B2 (2026-09-02) : `track_count === 0` = « aucun fichier audio reconnu »,
  // badge textuel « 0 audio ». Précédence : échec > vide > suspension — le vide est une
  // information corrective (mauvais dossier probable), la suspension un choix.
  it("aucun fichier reconnu : classe --empty, badge « 0 audio », motif dans le title", () => {
    const s = src({ track_count: 0 });
    const r = railRowState(s, [s], false, undefined);
    expect(r.rowClass).toContain("sift-rail-src--empty");
    expect(r.badge).toBe("0 audio");
    expect(r.title).toContain("aucun fichier audio reconnu");
  });

  it("l'échec PRIME sur le vide : un dossier inaccessible n'affiche pas « 0 audio »", () => {
    const s = src({ track_count: 0, accessible: false });
    const r = railRowState(s, [s], false, undefined);
    expect(r.rowClass).toContain("sift-rail-src--error");
    expect(r.rowClass).not.toContain("sift-rail-src--empty");
    expect(r.badge).toBe("");
  });

  it("le vide PRIME sur la suspension : le badge « 0 audio » se lit même suspendue", () => {
    const s = src({ track_count: 0, watched: false });
    const r = railRowState(s, [s], false, undefined);
    expect(r.rowClass).toContain("sift-rail-src--empty");
    expect(r.rowClass).not.toContain("sift-rail-src--suspended");
    expect(r.badge).toBe("0 audio");
  });

  it("un dossier traité (track_count > 0, pending 0) reste neutre : badge vide, pas --empty", () => {
    const s = src({ track_count: 5, pending_count: 0 });
    const r = railRowState(s, [s], false, undefined);
    expect(r.rowClass).not.toContain("sift-rail-src--empty");
    expect(r.badge).toBe("");
  });

  it("échappe un color_key adverse : la base ne contraint PAS les 5 valeurs (audit 2026-08-20)", () => {
    // `set_source_color` (ipc.rs) écrit la valeur telle quelle — aucune validation, aucun CHECK
    // SQL. Le seul verrou est le menu du frontend, donc ce champ se traite comme non fiable.
    const s = src({ color_key: `"><script>x</script>` });
    const html = sourceEntryHtml(s, [s], false, undefined);
    expect(html).not.toContain("<script>");
    expect(html).toContain("&quot;&gt;&lt;script&gt;");
  });
});

describe("sourceEntryHtml — échappement", () => {
  it("échappe le motif d'échec dans le title", () => {
    const s = src();
    const html = sourceEntryHtml(s, [s], false, `<img src=x onerror=alert(1)>`);
    expect(html).not.toContain("<img");
    expect(html).toContain("&lt;img");
  });

  it("échappe le nom de dossier affiché", () => {
    const s = src({ path: `C:\\music\\<b>"pièges"</b>` });
    const html = sourceEntryHtml(s, [s], false, undefined);
    expect(html).not.toContain("<b>");
    expect(html).toContain("&lt;b&gt;");
    expect(html).toContain("&quot;pièges&quot;");
  });
});

// Le rail se met à jour EN PLACE pendant un scan (issue #42) : `rail-sources.ts` garde ses nœuds
// et n'écrit que des valeurs. Les trois contrats ci-dessous sont ce dont ce chemin dépend, et
// chacun casse en SILENCE — pas d'exception, juste un rail qui ment.
describe("railRowState — contrat du chemin mutation", () => {
  it("rend du texte BRUT, jamais pré-échappé : le chemin mutation écrit des propriétés DOM", () => {
    // `textContent`/`title`/`className` ne parsent rien. Pré-échapper ici afficherait « &lt;b&gt; »
    // à l'écran, dans le rail, en clair. C'est l'inverse exact du bug d'échappement habituel — et
    // c'est pourquoi ce test regarde `not.toContain("&")` là où les autres exigent des entités.
    // Dernier segment SANS `/` : `baseName` découpe sur les deux séparateurs, donc un `</b>` dans
    // le nom couperait le libellé et masquerait ce qu'on mesure ici.
    const s = src({ path: `C:\\music\\<b>"pièges"` });
    const r = railRowState(s, [s], false, `<img src=x>`);
    expect(r.label).toBe(`<b>"pièges"`);
    expect(r.label).not.toContain("&lt;");
    expect(r.title).toContain(`<img src=x>`);
    expect(r.title).not.toContain("&lt;");
  });

  it("le badge est une chaîne VIDE à zéro, jamais absent — sinon la ligne n'est plus mutable", () => {
    // Un span toujours présent se met à jour par `textContent`. S'il disparaissait à zéro, passer
    // de 0 à 1 exigerait de CRÉER un nœud, donc de reconstruire — ce que tout ce chemin évite.
    // `.nav-badge:empty` (styles.css) replie la pastille, comme pour le badge de Revue.
    expect(railRowState(src({ pending_count: 0 }), [src()], false, undefined).badge).toBe("");
    expect(railRowState(src({ pending_count: 12 }), [src()], false, undefined).badge).toBe("12");
  });

  it("le marqueur actif vit dans rowClass, avec la grammaire .nv du rail", () => {
    const s = src();
    expect(railRowState(s, [s], true, undefined).rowClass).toBe("nv sift-rail-src on");
    expect(railRowState(s, [s], false, undefined).rowClass).toBe("nv sift-rail-src");
  });

  it("dotClass porte la classe de base ET la teinte : le chemin mutation écrase className entier", () => {
    // `dot.className = r.dotClass` remplace tout. Si `dotClass` oubliait `sift-rail-src-dot`, la
    // pastille perdrait sa taille et sa forme au premier tick de scan, pas au rendu initial.
    const r = railRowState(src(), [src()], false, undefined);
    expect(r.dotClass).toMatch(/^sift-rail-src-dot sift-rail-src-dot-(indigo|purple|pink|teal|yellow)$/);
  });
});

describe("sourceEntryHtml — structure exigée par le chemin mutation", () => {
  it("émet TOUJOURS les trois enfants, dans l'ordre pastille → libellé → badge", () => {
    // `rail-sources.ts` marche `firstElementChild` → `nextElementSibling` → `nextElementSibling`.
    // Cet ordre est un contrat entre deux fichiers, invisible depuis chacun d'eux pris seul.
    const s = src({ pending_count: 0 });
    const html = sourceEntryHtml(s, [s], false, undefined);
    const children = html.match(/<span[^>]*>/g) ?? [];
    expect(children).toHaveLength(3);
    expect(children[0]).toContain("sift-rail-src-dot");
    expect(children[1]).toBe("<span>");
    expect(children[2]).toContain("nav-badge");
  });

  it("le badge est présent même à zéro en attente", () => {
    const s = src({ pending_count: 0 });
    expect(sourceEntryHtml(s, [s], false, undefined)).toContain('<span class="nav-badge"></span>');
  });
});

describe("railShapeKey — ce qu'une mise à jour en place ne rattrape PAS", () => {
  const a = src({ id: 1, path: "C:\\a" });
  const b = src({ id: 2, path: "C:\\b" });

  it("ignore tout ce que la mutation sait écrire — sinon le scan reconstruirait à chaque tick", () => {
    // C'est LE test du correctif : pendant un scan, `pending_count` avance en permanence. S'il
    // entrait dans la clé, la forme changerait à chaque tick et on reconstruirait le rail 6 fois
    // par seconde — le bug d'origine, réintroduit par la porte de derrière.
    const base = railShapeKey([a, b]);
    expect(railShapeKey([{ ...a, pending_count: 999 }, b])).toBe(base);
    expect(railShapeKey([{ ...a, track_count: 0 }, b])).toBe(base);
    expect(railShapeKey([{ ...a, watched: false }, b])).toBe(base);
    expect(railShapeKey([{ ...a, accessible: false }, b])).toBe(base);
    expect(railShapeKey([{ ...a, color_key: "teal" }, b])).toBe(base);
  });

  it("change dès qu'il faut créer, retirer ou déplacer un nœud", () => {
    const base = railShapeKey([a, b]);
    expect(railShapeKey([a])).not.toBe(base); // retrait
    expect(railShapeKey([a, b, src({ id: 3 })])).not.toBe(base); // ajout
    expect(railShapeKey([b, a])).not.toBe(base); // réordonnancement
  });

  it("une liste vide a sa propre clé : la section montre alors un message, pas des lignes", () => {
    expect(railShapeKey([])).toBe("");
    expect(railShapeKey([])).not.toBe(railShapeKey([a]));
  });
});

describe("baseName", () => {
  it("prend le dernier segment, séparateurs Windows et POSIX confondus", () => {
    expect(baseName("C:\\music\\incoming")).toBe("incoming");
    expect(baseName("/home/dj/promos/")).toBe("promos");
    expect(baseName("solo")).toBe("solo");
  });
});
