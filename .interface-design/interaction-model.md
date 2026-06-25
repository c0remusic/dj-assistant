# Sift — Interaction Model (loi comportementale)

> Compagnon runtime de `system.md` (qui couvre le visuel). Ici : **comportement** —
> binding UI↔code, modèle dual Function/Intent, et la machine d'états déterministe.
> Règle : si un comportement ne s'écrit pas comme transition d'état, il n'existe pas dans Sift.
> Tokens : voir le set Penpot **System** (space xs/sm/md/lg/xl/xxl = 4/8/12/16/24/32 ·
> radius sharp/default/soft/pill = 4/6/10/999 · height compact/default/comfortable/large = 32/36/40/44).

## 1. Modèle dual (source de vérité)
- 🟢 **FUNCTION** — existe dans le runtime (`generate_handler!`), exécutable.
- 🟡 **INTENT** — présent dans la maquette = intention produit validée, **pas encore codé** :
  visible mais **non exécutable**, marqué « pending » (cf. §6), apparaît dans la palette section *In Development*.
- 🔴 **PROPOSED** — ni code ni maquette : proposé, jamais exécuté.
- ⚪ **NOOP** — affichage / état UI pur (sélection, mode, hover), aucun call backend.

`UI = Function Registry (🟢) + Intent Layer (🟡) + State Model`.

## 2. Function Registry (38 commandes réelles — src-tauri `generate_handler!`)
- **Sources** : add_source · list_sources · remove_source · set_source_watched · rescan_source · import_paths
- **Queue/Analyse** : list_queue · analyze_path · analysis_progress · playback_url
- **Filing** : reconcile · file_track · file_batch · reject_track · trash_track · undo_last · revert_batch ·
  requeue_track · restore_track · purge_trash · find_duplicate · list_journal · list_ecartes
- **Bins** : list_bins · create_bin
- **Identification** : identify · apply_identity_cmd
- **Library** : list_library · library_folders · update_metadata
- **Settings/Système** : get_setting · set_setting · app_info · db_health · ffmpeg_version · open_url · report_smoke

## 3. UI ↔ Function binding (écran Review)
| UI | type | boundAction | stateEffects |
|---|---|---|---|
| ▶ Play | 🟢 | playback_url(path) | — (média) |
| Move and Encode (CTA Detail) | 🟢 | file_track(id, bin) | Sel:remove · Focus:next · +journal |
| Move selection (n) (CTA Batch) | 🟢 | file_batch(ids, bin) | Sel:EMPTY · Focus:next |
| Discard (Detail) | 🟢 | reject_track(id) | Focus:next · →Écartés |
| Undo last filing | 🟢 | undo_last() | restore · Focus→it |
| Destination picker | 🟢 | list_bins/create_bin | bin=UI state (consommé par file_*) |
| Change (Discogs) | 🟢 | identify→apply_identity_cmd | maj identité |
| Verdict badges / Proof | ⚪ | disclosure (données analyze_path/identify/find_duplicate) | Expansion |
| Segment Discarded | 🟢 | list_ecartes (+restore/requeue) | mode vue |
| Select all · toggle Detail\|Batch · sélection | ⚪ | — | Selection/Mode |

## 4. Intent layer (🟡 — pending implementation)
| UI maquette | Fonction attendue | Note |
|---|---|---|
| Segment **Trashed** | `list_trashed()` | seuls trash_track/purge_trash existent |
| **Discard (n)** Batch | `reject_batch(ids)` | seul reject_track unitaire existe |
| Nav **Rekordbox** | `export_to_rekordbox(sel, target)` | groupe EXPORT |
| Nav **USB Drive** | `export_to_usb(sel, target)` | groupe EXPORT |

Ces éléments **restent dans l'UI**, jamais supprimés, jamais exécutés comme réels.

## 5. Machine d'états (déterministe)
```
FocusState     = NONE | QUEUE_ROW(id) | DEST_INPUT | GENRES_INPUT | PANEL(id) | COMMAND_PALETTE
SelectionState = EMPTY | SINGLE(id) | MULTI(id[])
ModeState      = DEFAULT | SEARCH | COMMAND_PALETTE | DETAIL | SELECTION
ExpansionState = CLOSED | DROPDOWN_OPEN(id) | ACCORDION_OPEN(id) | POPOVER_OPEN(id)
```
Transitions clés `(state, action) → newState` :
```
(Focus=QUEUE_ROW(A), ArrowDown)         → Focus=QUEUE_ROW(B)
(Focus=QUEUE_ROW(A), Tab)               → Focus=PANEL(inspector)
(Mode=DETAIL, Toggle)                   → Mode=SELECTION ;  (Mode=SELECTION, Toggle) → Mode=DETAIL
(Sel=EMPTY, Space@ROW(A)) [SELECTION]   → Sel=MULTI([A])
(Sel=MULTI(xs), Space@B)                → Sel=toggle(xs,B) ;  (Sel=*, SelectAll) → MULTI([visible])
(Exp=CLOSED, Click(DestPicker))         → Exp=DROPDOWN_OPEN(dest)  // overlay, ne pousse pas le layout
(Exp=CLOSED, Click(badge))              → Exp=ACCORDION_OPEN(proof) // inline
(Mode=*, Cmd+K)                         → Mode=COMMAND_PALETTE, Focus=COMMAND_PALETTE
```
Actions liées :
```
🟢 (Focus=ROW(A), Mode=DETAIL, Enter)        → file_track(A,bin)  ⇒ Sel:remove(A)·Focus:next·+journal
🟢 (Mode=SELECTION, Sel=MULTI(xs), Enter)    → file_batch(xs,bin) ⇒ Sel:EMPTY·Focus:next
🟢 (Focus=ROW(A), Backspace)                 → reject_track(A)    ⇒ Focus:next·A→Écartés
🟢 (*, Cmd+Z)                                → undo_last()
🟡 (Mode=SELECTION, Sel=MULTI(xs), Discard)  → INTENT reject_batch(xs)
🟡 (Click Trashed)                           → INTENT list_trashed()
🟡 (Click Rekordbox|USB)                     → INTENT export_to_*()
```
**Priorité (non négociable)** : Mode > Focus > Selection > Expansion > hover.
Conflits : Mode override tout · Focus override Selection · Expansion n'override jamais Mode
(Esc ferme d'abord l'expansion, puis sort du mode).
**Snapshot** toujours représentable :
`{ focus, selection[], mode, expansion }`.

## 6. Convention de marquage 🟡 PENDING (UI + maquette — RÉFÉRENCE PARTAGÉE)
Tout élément 🟡 INTENT, en code comme dans Penpot, est rendu **« pending implementation »** ainsi :
- **opacité 0.5** sur le label (et son icône) → signale non-actif ;
- **point ambre** 5×5, fill `#dda63f` (sémantique « doute/pending »), radius pill, placé à `space.sm` (8px) après le label ;
- **non-interactif** : l'action est INTENT, aucune exécution runtime ; au clic → « not implemented yet ».
Éléments concernés actuellement : Trashed (segment) · Discard (n) (Batch) · Rekordbox · USB Drive.

## 7. Clavier (Linear-like)
`↑/↓` focus · `Space` play | toggle-select · `⏎` File · `⌫` Discard · `⌘Z` undo · `Tab` zone suivante · `⌘K` palette (🟡/🔴).

**État front réel** (`filing.ts` `installFilingKeys`, 2026-06-25) : 🟢 `↑/↓` (parcourt la
queue live), `Space` play, `⏎` File, `⌫` Discard (alias `X`), `I` Identify, `⌘Z` undo.
🔴 pas encore : `Tab` (zone), `⌘K` (palette), `Space` toggle-select (mode batch absent).
