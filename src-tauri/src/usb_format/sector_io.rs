//! Adaptateur qui n'émet vers le support QUE des lectures et écritures alignées sur le secteur.
//!
//! Un handle de volume Windows (`\\.\I:`) n'accepte pas une écriture de 3 octets à l'offset 517 :
//! il veut des multiples entiers de la taille de secteur, à des offsets multiples de celle-ci.
//! `fatfs`, lui, écrit comme dans un fichier — quelques octets ici, un en-tête là.
//!
//! Cet adaptateur fait le pont : il garde une FENÊTRE de secteurs en mémoire, applique dessus les
//! écritures partielles, et ne pousse vers le support que des secteurs complets. Une écriture qui
//! commence au milieu d'un secteur relit d'abord la fenêtre (lecture-modification-écriture), sinon
//! les octets voisins seraient écrasés par du vide.
//!
//! **Ce n'était pas une optimisation.** Le premier formatage réel a échoué ici, sur un disque de
//! 500 Go : partition créée, FAT jamais écrite, volume laissé RAW. C'était l'hypothèse que j'avais
//! notée sans la vérifier — « l'alignement n'est exigé que pour `FILE_FLAG_NO_BUFFERING` » — et
//! elle était fausse pour ce chemin.
//!
//! **La fenêtre, elle, EST une optimisation — mesurée le 2026-09-09.** La première forme gardait
//! UN secteur : chaque secteur touché coûtait un `seek` + `ReadFile` (relecture systématique, même
//! pour un secteur entièrement réécrit) puis un `seek` + `WriteFile`. Or `fatfs::format_volume`
//! zéro-remplit les deux FAT par tranches de 512 o (`fs.rs::write_zeros`) : sur le SSD de 500 Go
//! d'Antoine, clusters de 32 Kio, deux FAT de ~61 Mo = ~240 000 secteurs, donc ~480 000 appels
//! système sur un volume USB brut — plusieurs minutes, avec une étape « Écriture du système de
//! fichiers FAT32… » figée (« on a juste l'impression que l'écran est bloqué »). Ici : fenêtre de
//! [`DEFAULT_WINDOW_SECTORS`] secteurs (1 Mio), et la relecture ne se fait QUE si une écriture est
//! partielle ou non contiguë — une suite d'écritures de secteurs entiers remplit la fenêtre sans
//! toucher au support, puis part en UN `WriteFile`. Les deux FAT de 122 Mo = ~120 écritures.
use std::io::{Read, Result as IoResult, Seek, SeekFrom, Write};

/// Secteurs par fenêtre : 2048 × 512 o = 1 Mio. Assez grand pour que le zéro-remplissage des FAT
/// parte par gros blocs, assez petit pour rester un tampon anodin.
pub const DEFAULT_WINDOW_SECTORS: u64 = 2048;

/// Rappel de progression : octets poussés vers le support depuis la création, cumulés. Appelé à
/// chaque écriture réelle, donc au plus une fois par fenêtre — c'est à l'appelant de lisser.
pub type Progress = Box<dyn FnMut(u64)>;

/// Support tamponné, aligné sur `sector` octets.
pub struct SectorIo<T: Read + Write + Seek> {
    inner: T,
    sector: u64,
    /// Taille de la fenêtre en octets — un multiple de `sector`.
    window: u64,
    /// La fenêtre en mémoire. Son contenu n'est significatif que sur `dirty`, ou partout si
    /// `read_done`.
    buf: Vec<u8>,
    /// Index de la fenêtre chargée. `None` = rien de chargé.
    loaded: Option<u64>,
    /// La fenêtre a été relue depuis le support (donc `buf` reflète le disque hors `dirty`).
    read_done: bool,
    /// Plage `[lo, hi)` d'octets écrits dans `buf` depuis le chargement, s'il y en a.
    dirty: Option<(usize, usize)>,
    /// Position logique vue par l'appelant, en octets depuis le début du volume.
    pos: u64,
    /// Octets réellement poussés vers le support, cumulés — ce que rapporte `progress`.
    written: u64,
    progress: Option<Progress>,
}

impl<T: Read + Write + Seek> SectorIo<T> {
    pub fn new(inner: T, sector: u64) -> Self {
        Self::with_window(inner, sector, DEFAULT_WINDOW_SECTORS)
    }

    /// Fenêtre explicite, en secteurs. Les tests l'emploient pour prouver que la fenêtre est ce
    /// qui fait la différence ; la production garde [`DEFAULT_WINDOW_SECTORS`].
    pub fn with_window(inner: T, sector: u64, sectors: u64) -> Self {
        let sectors = sectors.max(1);
        SectorIo {
            inner,
            sector,
            window: sector * sectors,
            buf: vec![0u8; (sector * sectors) as usize],
            loaded: None,
            read_done: false,
            dirty: None,
            pos: 0,
            written: 0,
            progress: None,
        }
    }

    /// Branche un rappel de progression (octets cumulés poussés vers le support).
    pub fn with_progress(mut self, progress: Progress) -> Self {
        self.progress = Some(progress);
        self
    }

    /// Fait de `index` la fenêtre courante, après avoir poussé la précédente. Ne lit RIEN : la
    /// relecture est différée à [`Self::ensure_read`], qui ne sert qu'aux écritures partielles et
    /// aux lectures.
    fn switch_to(&mut self, index: u64) -> IoResult<()> {
        if self.loaded == Some(index) {
            return Ok(());
        }
        self.flush_buf()?;
        self.loaded = Some(index);
        self.read_done = false;
        self.dirty = None;
        Ok(())
    }

    /// Relit la fenêtre courante depuis le support, en réappliquant par-dessus ce qui a déjà été
    /// écrit dedans (`dirty`). Une fenêtre au-delà de la fin du support lit court : on complète
    /// de zéros plutôt que d'échouer, parce qu'écrire le tout premier secteur d'un volume vierge
    /// commence forcément par une lecture de ce qui n'existe pas encore.
    fn ensure_read(&mut self) -> IoResult<()> {
        if self.read_done {
            return Ok(());
        }
        let Some(index) = self.loaded else {
            return Ok(());
        };
        let mut fresh = vec![0u8; self.buf.len()];
        self.inner.seek(SeekFrom::Start(index * self.window))?;
        let mut read = 0usize;
        while read < fresh.len() {
            match self.inner.read(&mut fresh[read..]) {
                Ok(0) => break,
                Ok(n) => read += n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        if let Some((lo, hi)) = self.dirty {
            fresh[lo..hi].copy_from_slice(&self.buf[lo..hi]);
        }
        self.buf = fresh;
        self.read_done = true;
        Ok(())
    }

    /// Pousse vers le support la plage salie de la fenêtre, arrondie au secteur. Sans relecture
    /// préalable, la plage est alignée par construction (voir `write`) ; avec, les octets de
    /// bordure sont ceux du disque.
    fn flush_buf(&mut self) -> IoResult<()> {
        let Some((lo, hi)) = self.dirty.take() else {
            return Ok(());
        };
        let Some(index) = self.loaded else {
            return Ok(());
        };
        let s = self.sector as usize;
        let lo_s = lo - lo % s;
        let hi_s = hi.div_ceil(s) * s;
        if (lo_s != lo || hi_s != hi) && !self.read_done {
            // Impossible par la règle de `write`, mais si ça arrivait, écrire des zéros à la place
            // des voisins serait pire qu'une relecture de plus.
            self.dirty = Some((lo, hi));
            self.ensure_read()?;
            self.dirty = None;
        }
        self.inner
            .seek(SeekFrom::Start(index * self.window + lo_s as u64))?;
        self.inner.write_all(&self.buf[lo_s..hi_s])?;
        self.written += (hi_s - lo_s) as u64;
        if let Some(p) = self.progress.as_mut() {
            p(self.written);
        }
        Ok(())
    }
}

impl<T: Read + Write + Seek> Read for SectorIo<T> {
    fn read(&mut self, out: &mut [u8]) -> IoResult<usize> {
        if out.is_empty() {
            return Ok(0);
        }
        let index = self.pos / self.window;
        let offset = (self.pos % self.window) as usize;
        self.switch_to(index)?;
        self.ensure_read()?;
        let n = out.len().min(self.window as usize - offset);
        out[..n].copy_from_slice(&self.buf[offset..offset + n]);
        self.pos += n as u64;
        Ok(n)
    }
}

impl<T: Read + Write + Seek> Write for SectorIo<T> {
    fn write(&mut self, data: &[u8]) -> IoResult<usize> {
        if data.is_empty() {
            return Ok(0);
        }
        let index = self.pos / self.window;
        let offset = (self.pos % self.window) as usize;
        self.switch_to(index)?;
        let n = data.len().min(self.window as usize - offset);
        let end = offset + n;
        // La règle qui évite la relecture : une écriture qui commence et finit sur un secteur ET
        // prolonge la plage déjà salie (ou l'ouvre) ne laisse aucun octet du support à préserver
        // dans ce qu'on poussera. Tout autre cas relit d'abord.
        let s = self.sector as usize;
        let aligned = offset % s == 0 && end % s == 0;
        let contiguous = match self.dirty {
            None => true,
            Some((_, hi)) => offset == hi,
        };
        if !(aligned && contiguous) {
            self.ensure_read()?;
        }
        self.buf[offset..end].copy_from_slice(&data[..n]);
        self.dirty = Some(match self.dirty {
            None => (offset, end),
            Some((lo, hi)) => (lo.min(offset), hi.max(end)),
        });
        self.pos += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> IoResult<()> {
        self.flush_buf()?;
        self.inner.flush()
    }
}

impl<T: Read + Write + Seek> Seek for SectorIo<T> {
    fn seek(&mut self, from: SeekFrom) -> IoResult<u64> {
        self.pos = match from {
            SeekFrom::Start(n) => n,
            SeekFrom::Current(d) => self.pos.saturating_add_signed(d),
            // `End` demanderait la taille du support ; `write_fat32` passe `total_sectors`
            // explicitement pour ne jamais avoir besoin de la chercher.
            SeekFrom::End(d) => {
                let end = self.inner.seek(SeekFrom::End(0))?;
                end.saturating_add_signed(d)
            }
        };
        Ok(self.pos)
    }
}

impl<T: Read + Write + Seek> Drop for SectorIo<T> {
    fn drop(&mut self) {
        // Sans ça, la dernière fenêtre écrite resterait en mémoire et le système de fichiers serait
        // tronqué — assez pour qu'il ne monte pas.
        let _ = self.flush_buf();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::io::Cursor;
    use std::rc::Rc;

    /// Toute E/S poussée vers le support doit être alignée : c'est la seule raison d'être de ce
    /// type. Un support espion enregistre chaque appel et refuse ce qui ne l'est pas — et compte
    /// les appels, pour que le coût soit une assertion et pas un adjectif.
    struct AlignedOnly {
        data: Vec<u8>,
        pos: u64,
        sector: u64,
        violations: Rc<RefCell<Vec<String>>>,
        reads: Rc<RefCell<usize>>,
        writes: Rc<RefCell<usize>>,
    }

    /// Les compteurs de l'espion, lisibles après qu'il a été consommé par `SectorIo`.
    struct Counters {
        violations: Rc<RefCell<Vec<String>>>,
        reads: Rc<RefCell<usize>>,
        writes: Rc<RefCell<usize>>,
    }

    impl AlignedOnly {
        fn new(len: usize) -> (Self, Counters) {
            let c = Counters {
                violations: Rc::new(RefCell::new(Vec::new())),
                reads: Rc::new(RefCell::new(0)),
                writes: Rc::new(RefCell::new(0)),
            };
            let spy = AlignedOnly {
                data: vec![0u8; len],
                pos: 0,
                sector: 512,
                violations: c.violations.clone(),
                reads: c.reads.clone(),
                writes: c.writes.clone(),
            };
            (spy, c)
        }
    }

    impl Read for AlignedOnly {
        fn read(&mut self, out: &mut [u8]) -> IoResult<usize> {
            *self.reads.borrow_mut() += 1;
            if self.pos % self.sector != 0 || out.len() as u64 % self.sector != 0 {
                self.violations
                    .borrow_mut()
                    .push(format!("lecture {} @ {}", out.len(), self.pos));
            }
            let start = self.pos as usize;
            if start >= self.data.len() {
                return Ok(0);
            }
            let n = out.len().min(self.data.len() - start);
            out[..n].copy_from_slice(&self.data[start..start + n]);
            self.pos += n as u64;
            Ok(n)
        }
    }

    impl Write for AlignedOnly {
        fn write(&mut self, data: &[u8]) -> IoResult<usize> {
            *self.writes.borrow_mut() += 1;
            if self.pos % self.sector != 0 || data.len() as u64 % self.sector != 0 {
                self.violations.borrow_mut().push(format!(
                    "écriture {} @ {}",
                    data.len(),
                    self.pos
                ));
            }
            let start = self.pos as usize;
            if self.data.len() < start + data.len() {
                self.data.resize(start + data.len(), 0);
            }
            self.data[start..start + data.len()].copy_from_slice(data);
            self.pos += data.len() as u64;
            Ok(data.len())
        }
        fn flush(&mut self) -> IoResult<()> {
            Ok(())
        }
    }

    impl Seek for AlignedOnly {
        fn seek(&mut self, from: SeekFrom) -> IoResult<u64> {
            self.pos = match from {
                SeekFrom::Start(n) => n,
                SeekFrom::Current(d) => self.pos.saturating_add_signed(d),
                SeekFrom::End(d) => (self.data.len() as u64).saturating_add_signed(d),
            };
            Ok(self.pos)
        }
    }

    /// LE test : des écritures minuscules et désalignées ne doivent produire QUE des E/S alignées.
    /// C'est ce qui manquait au premier formatage réel, qui a laissé un disque RAW.
    #[test]
    fn unaligned_writes_reach_the_device_aligned() {
        let (spy, c) = AlignedOnly::new(4096);
        let mut io = SectorIo::new(spy, 512);
        io.seek(SeekFrom::Start(3)).expect("seek");
        io.write_all(b"abc").expect("write");
        io.seek(SeekFrom::Start(517)).expect("seek");
        io.write_all(b"defgh").expect("write");
        io.flush().expect("flush");
        drop(io);
        assert!(
            c.violations.borrow().is_empty(),
            "E/S non alignées: {:?}",
            c.violations.borrow()
        );
    }

    /// Une écriture partielle ne doit PAS effacer les octets voisins du même secteur : sans
    /// lecture-modification-écriture, chaque petite écriture zapperait 511 octets autour d'elle.
    #[test]
    fn a_partial_write_preserves_its_neighbours() {
        let mut backing = Cursor::new(vec![0xAAu8; 1024]);
        {
            let mut io = SectorIo::new(&mut backing, 512);
            io.seek(SeekFrom::Start(10)).expect("seek");
            io.write_all(b"XY").expect("write");
            io.flush().expect("flush");
        }
        let out = backing.into_inner();
        assert_eq!(&out[8..14], &[0xAA, 0xAA, b'X', b'Y', 0xAA, 0xAA]);
    }

    /// Deux écritures de secteurs entiers dans la même fenêtre, mais NON contiguës : le trou entre
    /// elles est du disque, pas du vide. La règle « aligné ET contigu » doit relire — muter
    /// `contiguous` en `true` fait tomber ce test (les 0xAA du trou deviennent 0).
    #[test]
    fn a_gap_between_two_aligned_writes_keeps_the_device_bytes() {
        let mut backing = Cursor::new(vec![0xAAu8; 4096]);
        {
            let mut io = SectorIo::new(&mut backing, 512);
            io.write_all(&[1u8; 512]).expect("write");
            io.seek(SeekFrom::Start(1536)).expect("seek");
            io.write_all(&[2u8; 512]).expect("write");
            io.flush().expect("flush");
        }
        let out = backing.into_inner();
        assert_eq!(out[0], 1);
        assert_eq!(out[1536], 2);
        assert_eq!(
            &out[512..1536],
            &[0xAAu8; 1024][..],
            "le trou doit rester du disque"
        );
    }

    #[test]
    fn writes_then_reads_round_trip_across_sectors() {
        let mut backing = Cursor::new(vec![0u8; 4096]);
        let payload: Vec<u8> = (0..1500u32).map(|i| (i % 251) as u8).collect();
        {
            let mut io = SectorIo::new(&mut backing, 512);
            io.seek(SeekFrom::Start(300)).expect("seek");
            io.write_all(&payload).expect("write");
            io.flush().expect("flush");
        }
        let mut io = SectorIo::new(&mut backing, 512);
        io.seek(SeekFrom::Start(300)).expect("seek");
        let mut back = vec![0u8; payload.len()];
        io.read_exact(&mut back).expect("read");
        assert_eq!(back, payload);
    }

    /// Relire ce qu'on vient d'écrire dans la MÊME fenêtre, avant tout flush : la relecture du
    /// support doit réappliquer la plage salie par-dessus, sinon on lirait l'ancien contenu.
    #[test]
    fn reading_back_within_the_dirty_window_sees_the_writes() {
        let mut backing = Cursor::new(vec![0xAAu8; 4096]);
        let mut io = SectorIo::new(&mut backing, 512);
        io.write_all(&[7u8; 512]).expect("write");
        io.seek(SeekFrom::Start(0)).expect("seek");
        let mut back = [0u8; 512];
        io.read_exact(&mut back).expect("read");
        assert_eq!(back, [7u8; 512]);
        // Et le voisin non écrit reste celui du disque.
        let mut next = [0u8; 4];
        io.read_exact(&mut next).expect("read");
        assert_eq!(next, [0xAA; 4]);
    }

    /// Le dernier secteur doit partir même sans `flush` explicite : `fatfs` rend la main sans
    /// toujours vider, et 512 octets manquants suffisent a rendre le volume non montable.
    #[test]
    fn dropping_flushes_the_last_sector() {
        let mut backing = Cursor::new(vec![0u8; 1024]);
        {
            let mut io = SectorIo::new(&mut backing, 512);
            io.seek(SeekFrom::Start(600)).expect("seek");
            io.write_all(b"Z").expect("write");
            // pas de flush : c'est le Drop qui doit s'en charger
        }
        assert_eq!(backing.into_inner()[600], b'Z');
    }

    /// Le coût, mesuré (2026-09-09). Le zéro-remplissage des FAT par `fatfs` = des milliers
    /// d'écritures de 512 o consécutives : elles ne doivent provoquer AUCUNE lecture du support et
    /// une écriture par fenêtre, pas une par secteur. Muter `DEFAULT_WINDOW_SECTORS` en 1, ou
    /// forcer `ensure_read` dans `write`, fait tomber l'une ou l'autre assertion.
    #[test]
    fn sequential_full_sector_writes_never_read_and_go_out_per_window() {
        let sectors = 4 * DEFAULT_WINDOW_SECTORS as usize; // 4 fenêtres pleines
        let (spy, c) = AlignedOnly::new(sectors * 512);
        let seen = Rc::new(RefCell::new(Vec::new()));
        let seen2 = seen.clone();
        {
            let mut io = SectorIo::new(spy, 512)
                .with_progress(Box::new(move |n| seen2.borrow_mut().push(n)));
            let zeros = [0u8; 512];
            for _ in 0..sectors {
                io.write_all(&zeros).expect("write");
            }
            io.flush().expect("flush");
        }
        assert!(
            c.violations.borrow().is_empty(),
            "{:?}",
            c.violations.borrow()
        );
        assert_eq!(
            *c.reads.borrow(),
            0,
            "une réécriture de secteurs entiers ne relit rien"
        );
        assert_eq!(
            *c.writes.borrow(),
            4,
            "une écriture par fenêtre de 1 Mio, pas une par secteur"
        );
        assert_eq!(
            *seen.borrow(),
            vec![1 << 20, 2 << 20, 3 << 20, 4 << 20],
            "la progression cumule les octets réellement poussés"
        );
    }
}
