# Audit complet (code + UX) — 2026-06-13

Trois agents de revue (sécurité, architecture, correctness) sur **tout** le code, + un audit
UX (heuristiques Krug/Nielsen) sur l'UI. Ce doc liste ce qui a été corrigé et ce qui est
**différé** (avec la raison), pour que rien ne se perde.

## ✅ Corrigé et mergé

**Sécurité**
- `analyze_path` exige désormais que le chemin existe dans `tracks` — n'est plus un oracle de
  lecture/décodage de fichier arbitraire depuis la webview. (HIGH H2)
- `tagging::write_tags` ne `expect()` plus (erreur propre sur fichier piégé). (LOW L2)

**Correctness**
- L'empreinte (`tracks.fingerprint`) est effacée quand le fichier change (le scanner re-pend) →
  plus de cache d'empreinte périmé. (HIGH)
- `purge_trash` balaie aussi les `trash` orphelins (sans action de corbeille vivante) → ne
  restent plus coincés dans Écartés. (HIGH)
- `encode` garde la **première** erreur ffmpeg (souvent la plus parlante). (LOW)

**Architecture**
- `new_batch_id` : compteur monotone → plus de collision d'ID de lot à la même milliseconde
  (classe de bug d'undo). 
- `db::open` : **WAL + busy_timeout** (prépare la sortie du modèle mono-connexion ; inoffensif
  aujourd'hui).

**Frontend / UX**
- `openFilingInto` s'interrompt à ses `await` si un autre morceau a été ouvert entre-temps →
  plus de pane du mauvais morceau.
- Boutons Ranger / Re-sourcer / Écarter : désactivés + « Rangement… » pendant l'action → plus
  de double-clic (et statut visible). (Heuristique Nielsen #1 visibilité)
- Écartés : bouton « Slsk » → « Copier le nom » + tooltip (jargon retiré). (Krug : noms clairs)

**Décision assumée :** `name_key` reste un espace-join (sans séparateur) volontairement — ça
permet à « Larry Heard - Mystery of Love » de matcher un fichier « larry_heard mystery of
love » (doublon de convention de nom courant). La collision théorique champ-vide est acceptée.

## ⏳ Différé — à faire avec toi / vérification (par priorité)

1. **[SÉCURITÉ — top] CSP + scope `assetProtocol`.** `tauri.conf.json` a `csp: null` et
   `assetProtocol.scope = ["**"]` → une éventuelle XSS pourrait lire tout le disque et
   l'exfiltrer. Fix = définir une CSP stricte (autorisant jsdelivr pour les icônes, `asset:`/
   `blob:` pour l'audio/waveform, `ipc:`) + restreindre le scope aux racines source/biblio (au
   runtime, car choisies par l'utilisateur). **Non fait la nuit : une CSP mal réglée peut
   écran-blanc l'app — à régler avec l'app sous les yeux.**
2. **[ARCHI — top] Casser le `Mutex<Connection>` unique.** Le scan récursif
   (`spawn_scan → scanner::reconcile`) tient le verrou pendant tout le walkdir → gèle workers
   d'analyse + UI pendant l'import d'un gros dossier. (L'encode a déjà été sorti du verrou ;
   le scan non.) Fix = pool de connexions r2d2 (WAL déjà posé) ou scan hors-verrou par lots.
   Gros changement structurel, à faire posément.
3. **[ARCHI] Découper `sift-live.ts`** (~500 l, 5 responsabilités) en `ecartes-view.ts`,
   `chrome.ts` (titlebar + lean style + drag-drop), `home-sources.ts` ; garder `sift-live`
   comme installeur fin. Refactor de maintenabilité (pas un bug) — risqué à l'aveugle.
4. **[ARCHI/PERF] Persister `name_key`** en colonne indexée (calculée au scan) → dédup en
   `GROUP BY` indexé au lieu d'un re-parse O(n) à chaque ouverture/refresh. Utile quand la
   biblio grossit ; nécessite une migration.
5. **[CORRECTNESS] `file_batch` tient le verrou pendant les encodes** (comme l'ancien
   file_track). Pas encore câblé à l'UI (on range 1 par 1) → différé jusqu'à ce que le batch
   serve.
6. **[SÉCURITÉ] `restore_track`/undo non contraints à une racine** : un fichier déposé peut
   être restauré n'importe où (par conception — il revient d'où il vient). Le vrai risque est
   l'XSS → couvert par #1 (CSP). Ne pas contraindre (casserait la restauration légitime).
7. **[ARCHI] Contrat d'augmentation implicite** (`window.__sift*` + ids DOM en dur partagés
   avec la démo `app.js`) : ajouter des assertions à l'install (erreur si un id requis manque)
   + marquer les ids/classes « porteurs » dans app.js.
8. **[ARCHI] Retirer les `#![allow(dead_code)]`** de modules désormais câblés pour faire
   ressortir le code réellement mort (ex. `file_batch`/`TRASH_PURGE_DAYS`).
9. **[CORRECTNESS] L'undo ne restaure pas les tags ré-écrits** (move conforme) — journaliser
   un `retag` ou snapshotter les tags d'origine ; à défaut le documenter côté UI.
10. **[ARCHI] Le front re-devine le rail** (lossless/lossy par extension) au lieu de le recevoir
    du backend — faire renvoyer le rail par `reconcile`/la file.

## UX — score & frictions restantes

État actuel **~7/10** (forces : review-first + undo partout, langage simple, signalement non
bloquant des doublons). Frictions notées non traitées :
- **Nav en icônes seules** (Accueil/Revue/Écartés) sans libellé visible — « mystery-meat »,
  surtout pour un utilisateur non-dev. Ajouter des libellés (touche la démo/`app.js` + le
  layout 46px — à faire avec toi).
- **Pas de filage clavier** : la maquette avait 1-9 / Entrée / Espace ; non recâblés au vrai
  classement. Gain d'efficacité DJ (heuristique #7).
- **Bouton agrandir** de la titlebar ne bascule pas en « restaurer » quand maximisé (mineur).
- Messages d'erreur de toast parfois bruts (`Échec : <raw>`) — à humaniser au cas par cas.

---

# Audit — round 2 (2026-06-14)

Deuxième passe (3 agents : sécurité / archi / correctness) après que plusieurs différés du
round 1 aient été livrés entre-temps (CSP stricte, scope asset, libellés nav, filage clavier
Entrée/Écarter/Espace, lecture AIFF native, cache de rapport, scan hors-verrou, scrollbars).

## ✅ Corrigé et mergé (round 2)

**Sécurité**
- `analyze_path` : la vérif « chemin connu » est désormais **inconditionnelle** — avant, elle
  était sautée quand `with_spectrogram=true` ou quand le cache était le sentinelle d'échec
  (`report_json=''`), rouvrant l'oracle de décodage de fichier arbitraire. (CRITICAL)
- `playback_url` : ré-encode si le WAV en cache (chemin temp prévisible) est **plus vieux que
  la source** — un fichier remplacé, ou un fichier squatté au nom prévisible, n'est plus servi
  périmé. (SEC-001)
- `esc()` (3 fichiers front) échappe aussi `'` → échappement complet des attributs HTML.

**Correctness**
- `loadDecoded` (lecteur AIFF) : s'interrompt à chacun de ses `await` si le morceau a changé
  (`ws !== currentWs`) → ne `loadBlob`/`load` plus jamais sur un wavesurfer détruit (crash au
  changement rapide de piste). (CRITICAL)
- `openReportInto` : jeton monotone `openSeq` → une analyse lente qui résout après un changement
  de piste n'écrase plus le pane du nouveau morceau. (HIGH)
- `reportCache` (front) : vidé sur `analysis:changed` → un fichier ré-analysé/remplacé n'est plus
  servi depuis le cache de session périmé (le backend/DB reste la source de vérité). (HIGH)
- `persist_result` (worker) : une écriture DB échouée (ex. `SQLITE_BUSY`) est désormais
  **journalisée en erreur** au lieu d'être avalée silencieusement — le morceau reste
  `analyzed_at=NULL` et est repris au prochain refill. (HIGH, mitigation)

## ⏳ Différé — round 2 (raison)

1. **[ARCHI — top, toujours] Pool de connexions DB.** Le scan a sa propre `Connection` (hors du
   `Mutex` partagé), donc l'UI ne gèle plus ; mais scan-écriture et worker-écriture peuvent
   encore se télescoper sur `SQLITE_BUSY` (mitigé par `busy_timeout=5000` + log ci-dessus, pas
   éliminé). Le vrai fix reste un pool r2d2 + retry/transaction. Gros changement, à faire posément.
2. **[ARCHI] Découper `sift-live.ts`** (god-module ~520 l) — refactor de maintenabilité, risqué
   à l'aveugle. (inchangé depuis round 1)
3. **[QUALITÉ-AUDIO] Lecture AIFF en 16-bit pour l'aperçu.** `loadDecoded` ré-encode en WAV
   16/44.1 pour wavesurfer → l'aperçu peut tronquer un AIFF 24-bit. C'est **voulu** (lecture
   native demandée, pas de transcode backend ; et c'est un aperçu, pas la sortie rangée). Ne PAS
   basculer sur un transcode backend unique sans ton aval (contredit « en natif »).
4. **[CORRECTNESS] `clean_stem`** : le découpage d'octets du numéro de piste en tête est sûr
   aujourd'hui (chiffres ASCII) — laissé tel quel, latent seulement.
5. Restent ouverts du round 1 : `name_key` indexé (#4), undo des tags ré-écrits (#9), rail
   renvoyé par le backend (#10), retrait des `#![allow(dead_code)]` (#8).
