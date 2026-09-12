//! Occupation par format, pour le graphique : la clé branchée (parcours du volume, mis en cache)
//! et la bibliothèque (simple agrégat en base, sans aucune entrée/sortie disque).

use crate::db;
use crate::usb_format::{self, RemovableDriveBackend};
use crate::volume_usage::{self, ExtUsage};
use rusqlite::Connection;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::State;

/// Ce que le graphique consomme. `free_bytes` vaut 0 pour la bibliothèque : elle n'a pas de
/// volume, donc pas d'espace libre — le frontend n'y dessine simplement pas de segment « libre ».
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UsageReport {
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub file_count: u64,
    pub buckets: Vec<ExtUsage>,
    /// Vrai quand rien n'a été parcouru. Affiché : sans ça, un cache faux serait indiscernable
    /// d'une mesure fraîche, et personne ne saurait quoi actualiser.
    pub from_cache: bool,
    /// Epoch en secondes du parcours qui a produit ces chiffres, cache ou non.
    pub scanned_at: i64,
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Relit le cache pour ce disque, et **ne le rend que s'il est encore vrai**.
///
/// L'invalidation se fait sur l'espace libre, jamais sur une durée : un cache d'hier peut être
/// exact, un cache d'il y a dix secondes peut être faux. Si l'espace libre du volume diffère de
/// celui enregistré, du contenu a été ajouté ou retiré, et la ventilation ne vaut plus rien.
pub(crate) fn read_cache(conn: &Connection, key: &str, free_bytes: u64) -> Option<UsageReport> {
    let row: Option<(i64, i64, i64, i64, String, i64)> = conn
        .query_row(
            "SELECT scanned_at, total_bytes, free_bytes, file_count, buckets_json, scheme_version
             FROM volume_usage WHERE volume_key = ?1",
            rusqlite::params![key],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            },
        )
        .ok();
    let (scanned_at, total, free, count, json, scheme) = row?;
    // Deux invalidations distinctes, et il en faut deux : l'espace libre attrape un contenu qui a
    // bouge, la version de schéma attrape une RÈGLE de classement qui a changé. Ni l'une ni l'autre
    // ne couvre le cas de l'autre.
    if free as u64 != free_bytes || scheme != volume_usage::BUCKET_SCHEME_VERSION {
        return None;
    }
    let buckets: Vec<ExtUsage> = serde_json::from_str(&json).ok()?;
    Some(UsageReport {
        total_bytes: total as u64,
        free_bytes: free as u64,
        file_count: count as u64,
        buckets,
        from_cache: true,
        scanned_at,
    })
}

pub(crate) fn write_cache(
    conn: &Connection,
    key: &str,
    report: &UsageReport,
) -> rusqlite::Result<()> {
    let json = serde_json::to_string(&report.buckets).unwrap_or_else(|_| "[]".to_string());
    conn.execute(
        "INSERT INTO volume_usage
            (volume_key, scanned_at, total_bytes, free_bytes, file_count, buckets_json,
             scheme_version)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(volume_key) DO UPDATE SET
            scanned_at=excluded.scanned_at, total_bytes=excluded.total_bytes,
            free_bytes=excluded.free_bytes, file_count=excluded.file_count,
            buckets_json=excluded.buckets_json, scheme_version=excluded.scheme_version",
        rusqlite::params![
            key,
            report.scanned_at,
            report.total_bytes as i64,
            report.free_bytes as i64,
            report.file_count as i64,
            json,
            volume_usage::BUCKET_SCHEME_VERSION
        ],
    )?;
    Ok(())
}

/// `"I:"` ou `"I:, J:"` -> les racines à parcourir. Un disque partitionné monte plusieurs lettres,
/// et sa taille comme son espace libre sont déjà la somme des volumes : la ventilation doit suivre.
pub(crate) fn scan_roots(mount: &str) -> Vec<PathBuf> {
    mount
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|letter| PathBuf::from(format!("{}\\", letter.trim_end_matches('\\'))))
        .collect()
}

/// Occupation du disque amovible `drive_id`, par format.
///
/// Synchrone comme toutes les commandes de ce dépôt (`file_batch` encode des fichiers entiers sur
/// le même modèle) : le parcours ne lit que des métadonnées, jamais un octet de contenu.
///
/// **Le verrou de la base n'est jamais tenu pendant le parcours.** On lit le cache, on relâche, on
/// parcourt, on reprend pour écrire. Tenir le `Mutex<Connection>` plusieurs secondes bloquerait le
/// pool d'analyse et le watcher, qui n'ont rien demandé.
#[tauri::command]
pub fn drive_usage(
    conn: State<'_, Mutex<Connection>>,
    drive_id: String,
    force_rescan: bool,
) -> Result<UsageReport, String> {
    let drives = usb_format::backend_for_this_os()
        .list()
        .map_err(|e| e.to_string())?;
    let drive = drives
        .into_iter()
        .find(|d| d.id == drive_id)
        .ok_or_else(|| usb_format::DRIVE_VANISHED.to_string())?;

    let roots = scan_roots(&drive.mount);
    if roots.is_empty() {
        return Err("Ce disque n'a aucun volume monté — rien à parcourir.".to_string());
    }

    if !force_rescan {
        let cached = {
            let c = db::lock_conn(&conn)?;
            read_cache(&c, &drive.identity, drive.free_bytes)
        };
        if let Some(report) = cached {
            return Ok(report);
        }
    }

    let mut per_root = Vec::new();
    for root in &roots {
        per_root.push(volume_usage::scan_volume(root).map_err(|e| e.to_string())?);
    }
    let buckets = volume_usage::merge(per_root);

    let report = UsageReport {
        total_bytes: drive.size_bytes,
        free_bytes: drive.free_bytes,
        file_count: buckets.iter().map(|b| b.file_count).sum(),
        buckets,
        from_cache: false,
        scanned_at: now_secs(),
    };

    {
        let c = db::lock_conn(&conn)?;
        if let Err(e) = write_cache(&c, &drive.identity, &report) {
            // Le cache est une optimisation : son échec ne doit pas priver l'utilisateur du
            // résultat qu'on vient de passer plusieurs secondes à calculer. Mais il se trace.
            log::error!("volume_usage: écriture du cache impossible: {e}");
        }
    }

    Ok(report)
}

/// Occupation de la bibliothèque, par format. Aucune entrée/sortie disque : les tailles sont déjà
/// en base depuis la migration v2, donc c'est un agrégat, pas un parcours.
#[tauri::command]
pub fn library_usage(conn: State<'_, Mutex<Connection>>) -> Result<UsageReport, String> {
    let conn = db::lock_conn(&conn)?;
    // `status = 'filed'`, EXACTEMENT le filtre de `library::list_filed`. Sans lui la requête
    // ramenait toute la table `tracks`, pending compris : l'écran Bibliothèque annonçait « 1 piste »
    // pendant que le graphique juste au-dessus affichait 148,8 Go d'AIFF. Deux chiffres vrais, deux
    // populations différentes, et rien pour le dire — vu en le faisant tourner le 2026-08-01.
    let mut stmt = conn
        .prepare(
            "SELECT path, size_bytes FROM tracks \
             WHERE status = 'filed' AND size_bytes IS NOT NULL",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
        .map_err(|e| e.to_string())?;

    let mut pairs: Vec<(String, u64)> = Vec::new();
    for row in rows {
        let Ok((path, size)) = row else { continue };
        // Seul le nom de fichier : `bucket_for` réserve sa règle « sous PIONEER/ » aux chemins
        // relatifs à une racine de volume, et un chemin de bibliothèque n'en est pas un.
        let name = Path::new(&path)
            .file_name()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(&path));
        pairs.push((volume_usage::bucket_for(&name), size.max(0) as u64));
    }

    let buckets = volume_usage::aggregate(pairs);
    Ok(UsageReport {
        total_bytes: buckets.iter().map(|b| b.bytes).sum(),
        // Une bibliothèque n'est pas un volume : pas d'espace libre à annoncer, donc pas de
        // segment « libre » dans la barre.
        free_bytes: 0,
        file_count: buckets.iter().map(|b| b.file_count).sum(),
        buckets,
        from_cache: false,
        scanned_at: now_secs(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem_db() -> Connection {
        let conn = Connection::open_in_memory().expect("open");
        db::run_migrations(&conn).expect("migrations");
        conn
    }

    fn report(free: u64) -> UsageReport {
        UsageReport {
            total_bytes: 1000,
            free_bytes: free,
            file_count: 3,
            buckets: vec![ExtUsage {
                ext: ".wav".to_string(),
                bytes: 700,
                file_count: 3,
            }],
            from_cache: false,
            scanned_at: 42,
        }
    }

    #[test]
    fn cache_round_trips() {
        let conn = mem_db();
        write_cache(&conn, "K", &report(500)).expect("write");
        let got = read_cache(&conn, "K", 500).expect("hit attendu");
        assert_eq!(got.buckets, report(500).buckets);
        assert_eq!(got.file_count, 3);
        assert!(got.from_cache, "un cache relu doit s'annoncer comme tel");
    }

    /// L'invalidation est le cœur du sujet : un octet d'espace libre en moins veut dire qu'un
    /// fichier a été ajouté, donc que la ventilation est périmée.
    #[test]
    fn changed_free_space_invalidates() {
        let conn = mem_db();
        write_cache(&conn, "K", &report(500)).expect("write");
        assert!(read_cache(&conn, "K", 499).is_none());
        assert!(read_cache(&conn, "K", 501).is_none());
    }

    /// Une ventilation calculée par une ancienne règle de classement doit être rejetée même si le
    /// disque n'a pas bougé d'un octet — l'espace libre ne peut pas détecter ça.
    #[test]
    fn an_older_bucket_scheme_invalidates() {
        let conn = mem_db();
        write_cache(&conn, "K", &report(500)).expect("write");
        conn.execute(
            "UPDATE volume_usage SET scheme_version = ?1 WHERE volume_key = 'K'",
            rusqlite::params![volume_usage::BUCKET_SCHEME_VERSION - 1],
        )
        .expect("downgrade");
        assert!(read_cache(&conn, "K", 500).is_none());
    }

    /// Les lignes écrites par la v17 n'avaient pas de colonne de version : la v18 leur pose 0, qui
    /// ne correspond à aucune version émise, donc elles sont recalculées. C'est voulu — ce sont
    /// exactement celles qui portent l'ancien découpage .aif/.aiff.
    #[test]
    fn rows_from_before_the_version_column_are_recomputed() {
        let conn = mem_db();
        write_cache(&conn, "K", &report(500)).expect("write");
        conn.execute(
            "UPDATE volume_usage SET scheme_version = 0 WHERE volume_key = 'K'",
            [],
        )
        .expect("legacy");
        assert!(read_cache(&conn, "K", 500).is_none());
    }

    #[test]
    fn unknown_key_is_a_miss_not_an_error() {
        let conn = mem_db();
        assert!(read_cache(&conn, "jamais-vu", 0).is_none());
    }

    /// Deux clés branchées successivement sur le même port ont des identités différentes ; la
    /// seconde ne doit pas hériter du cache de la première.
    #[test]
    fn two_disks_do_not_share_a_cache_entry() {
        let conn = mem_db();
        write_cache(&conn, "disque-A", &report(500)).expect("write");
        assert!(read_cache(&conn, "disque-B", 500).is_none());
        assert!(read_cache(&conn, "disque-A", 500).is_some());
    }

    #[test]
    fn rescanning_replaces_the_entry_instead_of_duplicating_it() {
        let conn = mem_db();
        write_cache(&conn, "K", &report(500)).expect("write");
        let mut second = report(400);
        second.file_count = 9;
        write_cache(&conn, "K", &second).expect("write 2");
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM volume_usage", [], |r| r.get(0))
            .expect("count");
        assert_eq!(n, 1, "la clé primaire doit écraser, pas empiler");
        assert_eq!(read_cache(&conn, "K", 400).expect("hit").file_count, 9);
    }

    #[test]
    fn scan_roots_splits_a_multi_volume_disk() {
        assert_eq!(scan_roots("I:"), vec![PathBuf::from("I:\\")]);
        assert_eq!(
            scan_roots("I:, J:"),
            vec![PathBuf::from("I:\\"), PathBuf::from("J:\\")]
        );
    }

    /// Un disque RAW n'a aucune lettre : la commande doit refuser explicitement plutôt que de
    /// parcourir une racine vide et rendre un graphique vide, qui se lirait comme « clé vide ».
    #[test]
    fn scan_roots_of_an_unmounted_disk_is_empty() {
        assert!(scan_roots("").is_empty());
        assert!(scan_roots("  ").is_empty());
    }
}
