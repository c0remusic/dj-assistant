<!--
Sift est un logiciel propriétaire et n'accepte pas de pull requests
extérieures. Les issues restent ouvertes pour les bugs et les demandes.
-->

## Ce que ça change

<!-- Une ou deux phrases. Le comportement observable, pas la liste des fichiers. -->

Corrige #

## Pourquoi ainsi

<!--
La raison du choix, et ce qui a été écarté. C'est la partie qui se perd et
qu'on regrette dans six mois.
-->

## La preuve

<!--
Comment vous savez que ça marche. Pour un correctif : le test qui échouait
avant et passe après. Un test qu'on n'a pas vu échouer ne prouve rien.
-->

## Vérifications

- [ ] `cargo test --manifest-path src-tauri/Cargo.toml`
- [ ] `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- [ ] `cargo fmt --manifest-path src-tauri/Cargo.toml --check`
- [ ] `npx tsc --noEmit`
- [ ] Si l'interface est touchée : vérifié dans la vraie fenêtre `tauri dev`, pas seulement dans le navigateur — les modules live ne s'exécutent que dans le shell Tauri.
- [ ] Si la base Rekordbox est touchée : sauvegarde vérifiée avant écriture, et round-trip contrôlé.
