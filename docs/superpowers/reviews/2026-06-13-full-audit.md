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
