import { describe, expect, it } from "vitest";
import { assertSpectrogramLength } from "../frontend/ipc";
import { decodeB85 } from "../frontend/b85";
import type { Spectrogram } from "../shared/contracts";

// `frontend/b85.ts` sait dire « ces caractères ne sont pas du base85 ». Il ne sait pas dire
// « il en manque » : sa seule garde de longueur est le reste d'UN caractère (`UnexpectedEof`),
// donc toute troncature qui tombe sur un multiple de 5 caractères le traverse intacte. Et
// `report-view.ts` lit ensuite la grille par `frames * bins` — les octets manquants rendaient
// `undefined`, qu'un `|| 0` transformait en 0, c'est-à-dire -100 dBFS, c'est-à-dire du silence.
// Spectrogramme décalé, fin en noir, aucune erreur nulle part.
//
// C'est ce trou que `assertSpectrogramLength` bouche, au seul point de décodage du frontend
// (`ipc.ts::analyzePath`). Ce fichier tient son comportement.

/** Construit un spectrogramme cohérent : `frames * bins` octets, comme le backend en produit
 *  (`spectrum.rs:264-268`). `hz_per_bin`/`sec_per_frame` ne jouent aucun rôle dans la garde. */
function grille(frames: number, bins: number, longueur = frames * bins): Spectrogram {
  return {
    frames,
    bins,
    hz_per_bin: 21.5,
    sec_per_frame: 0.05,
    mag_db: new Uint8Array(longueur),
  };
}

/** Le message de l'erreur levée par `fn`, ou `null` si elle n'a pas levé. Passe par un capture
 *  plutôt que `toThrow(/…/)` pour pouvoir affirmer plusieurs choses sur le MÊME message. */
function messageLevé(fn: () => void): string | null {
  try {
    fn();
    return null;
  } catch (e) {
    return e instanceof Error ? e.message : String(e);
  }
}

describe("assertSpectrogramLength", () => {
  it("laisse passer une grille de la taille annoncée", () => {
    expect(() => assertSpectrogramLength(grille(7, 53))).not.toThrow();
  });

  it("laisse passer la sentinelle « pas de spectrogramme »", () => {
    // Le cas le PLUS fréquent en production : tous les appels sauf un passent
    // `withSpectrogram=false` (prefetch, ouverture de l'écran Revue, modale, self-test), et
    // `spectrum.rs:230-238` renvoie alors `frames: 0, bins: 0, mag_db: vec![]`. La sentinelle
    // satisfait l'invariant (`0 === 0*0`), elle n'y échappe pas — donc aucune exemption à écrire.
    // Une garde qui lèverait ici casserait l'application entière au lieu d'un collapse.
    expect(() => assertSpectrogramLength(grille(0, 0))).not.toThrow();
  });

  it("lève sur une grille tronquée", () => {
    // 7*53 = 371 attendus, 368 reçus. L'écart de 3 octets est le cas réel : il décale la grille
    // et la termine en noir sans que rien ne le signale.
    expect(() => assertSpectrogramLength(grille(7, 53, 368))).toThrow();
  });

  it("lève aussi sur une grille TROP LONGUE", () => {
    // L'indexation continuerait de fonctionner — mais un surplus veut dire que `frames`/`bins` ne
    // décrivent pas la grille qui les accompagne, donc que ce qui est affiché n'est pas ce que
    // l'analyse a mesuré. C'est le même désaccord de contrat, pas un cas bénin.
    expect(() => assertSpectrogramLength(grille(7, 53, 400))).toThrow();
  });

  it("cite les deux longueurs dans son message", () => {
    // Sans elles, le message dit qu'il y a un problème sans dire lequel : impossible de savoir si
    // la chaîne a été tronquée sur le fil ou si `frames`/`bins` mentent. Même exigence que
    // `valueAt` (`b85.ts:33`), qui cite le caractère fautif ET sa position.
    // Le FORMAT reste libre — seule la présence des deux nombres est tenue ici.
    const msg = messageLevé(() => assertSpectrogramLength(grille(7, 53, 368)));
    expect(msg).not.toBeNull();
    expect(msg).toContain("371"); // attendu (7 * 53)
    expect(msg).toContain("368"); // reçu
  });

  it("rattrape une troncature que le décodeur base85 laisse passer", () => {
    // La démonstration du trou, pas une reformulation : cette chaîne est du base85 parfaitement
    // valide (groupe plein, aucun reste), `decodeB85` ne lève pas et rend 4 octets. Si le rapport
    // qui l'accompagne annonce 2 frames × 3 bins, il en manque 2 — et rien, avant cette garde,
    // ne le remarquait.
    const octets = decodeB85("009C6");
    expect(octets).toHaveLength(4);
    expect(() => assertSpectrogramLength({ ...grille(2, 3), mag_db: octets })).toThrow();
  });
});
