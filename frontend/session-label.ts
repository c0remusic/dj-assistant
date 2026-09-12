// Libellé lisible d'un `session_id` backend — module pur, sans DOM, testable en env Node.
//
// `session_id` = "{millis}-{pid}" (`lib.rs`). On dérive un libellé depuis la partie millis ; un
// format inattendu rend l'ID brut plutôt qu'un plantage. Extrait de `journal.ts` le 2026-09-08
// (déclinaison #24, cinquième écran) : les groupes de candidats de Rekordbox affichaient
// l'identifiant brut « 1788016612965-23936 » là où le Journal disait « Session de 01h24 ». Une
// session porte UN nom dans toute l'app — c'est le point du partage.

export function sessionStart(sessionId: string): Date | null {
  const millis = Number(sessionId.split("-")[0]);
  if (!Number.isFinite(millis) || millis <= 0) return null;
  const d = new Date(millis);
  return Number.isNaN(d.getTime()) ? null : d;
}

/** `withDate` : « Session du 08/09/2026 01h24 » (liste sans regroupement par jour — Rekordbox,
 *  premier niveau du Journal) ; sinon « Session de 01h24 » (sous un groupe de jour). */
export function sessionLabel(sessionId: string | null, withDate: boolean): string {
  if (sessionId == null) return "Hors session";
  const d = sessionStart(sessionId);
  if (!d) return sessionId;
  const hh = `${String(d.getHours()).padStart(2, "0")}h${String(d.getMinutes()).padStart(2, "0")}`;
  if (!withDate) return `Session de ${hh}`;
  const dd = String(d.getDate()).padStart(2, "0");
  const mm = String(d.getMonth() + 1).padStart(2, "0");
  return `Session du ${dd}/${mm}/${d.getFullYear()} ${hh}`;
}
