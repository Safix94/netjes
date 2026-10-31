use crate::categories::{Category, folder_name};
use crate::planner::{PlannedMove, unique_dest};
use serde::Serialize;
use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

const OUTSIDE: &str = "Het bestand ligt buiten de map";
const INVALID_DEST: &str = "Ongeldige bestemming";
const NO_FREE_NAME: &str = "Geen vrije bestandsnaam gevonden";
/// Een rename kan een vrije naam verliezen aan een ander proces; zoveel nieuwe namen proberen we.
const MAX_ATTEMPTS: usize = 10;

/// Resultaat van één verplaatsing. `dest` is de definitieve bestemming na conflictoplossing.
#[derive(Clone, Serialize, Debug)]
pub struct MoveOutcome {
    pub id: u32,
    pub ok: bool,
    pub dest: Option<String>,
    pub error: Option<String>,
}

/// Maakt de ontbrekende categoriemappen rechtstreeks onder `root` aan en geeft enkel die terug
/// die nu nieuw zijn. Zo kan het logboek ze achteraf weer opruimen als ze leeg blijven.
pub fn ensure_folders(root: &Path, cats: &[Category]) -> io::Result<Vec<PathBuf>> {
    let mut created = Vec::new();
    for &cat in cats {
        let folder = root.join(folder_name(cat));
        match fs::create_dir(&folder) {
            Ok(()) => created.push(folder),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
    }
    Ok(created)
}

/// Verplaatst één bestand met één rename naar een vrije naam. Op Windows weigert die rename zelf
/// een bestaand doel (`rename_no_replace`), dus ook een bestand dat pas tijdens de verplaatsing
/// verschijnt wordt nooit overschreven; dan kiest de volgende poging een andere naam. Volgt geen
/// symlinks en blijft binnen de gekozen map (die volgt uit `dest`: `root/<map>/<naam>`).
/// De doelmap moet al bestaan; daar zorgt de aanroeper voor via `ensure_folders`.
pub fn move_one(m: &PlannedMove) -> MoveOutcome {
    match try_move(m) {
        Ok(dest) => MoveOutcome {
            id: m.id,
            ok: true,
            dest: Some(dest.to_string_lossy().into_owned()),
            error: None,
        },
        Err(error) => MoveOutcome {
            id: m.id,
            ok: false,
            dest: None,
            error: Some(error),
        },
    }
}

/// Verplaatst alles na elkaar. Een mislukking stopt de rest niet; elk bestand krijgt zijn eigen uitkomst.
pub fn move_many(moves: &[PlannedMove]) -> Vec<MoveOutcome> {
    moves.iter().map(move_one).collect()
}

/// Hernoemt zonder ooit een bestaand doel te vervangen: de controle zit in de systeemaanroep zelf
/// en niet in een aparte stap ervoor. `MoveFileExW` met vlaggen 0 vervangt niets, kopieert niet
/// naar een andere schijf en geeft `AlreadyExists` als het doel er is.
#[cfg(windows)]
fn rename_no_replace(src: &Path, dest: &Path) -> io::Result<()> {
    use std::iter::once;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::MoveFileExW;

    let wide = |p: &Path| -> Vec<u16> { p.as_os_str().encode_wide().chain(once(0)).collect() };
    let (src, dest) = (wide(src), wide(dest));
    // SAFETY: beide buffers zijn met 0 afgesloten en leven langer dan de aanroep.
    let moved = unsafe { MoveFileExW(src.as_ptr(), dest.as_ptr(), 0) };
    if moved == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// Buiten Windows bestaat er in `std` geen rename zonder vervangen; Netjes draait enkel op Windows,
/// dit houdt de bibliotheek compileerbaar.
#[cfg(not(windows))]
fn rename_no_replace(src: &Path, dest: &Path) -> io::Result<()> {
    fs::rename(src, dest)
}

fn try_move(m: &PlannedMove) -> Result<PathBuf, String> {
    let src = Path::new(&m.src);
    let dest = Path::new(&m.dest);
    let dest_dir = dest.parent().ok_or(INVALID_DEST)?;
    let root =
        fs::canonicalize(dest_dir.parent().ok_or(INVALID_DEST)?).map_err(|e| e.to_string())?;

    let src_dir = src.parent().ok_or(OUTSIDE)?;
    let src_dir = fs::canonicalize(src_dir).map_err(|e| e.to_string())?;
    if !src_dir.starts_with(&root) {
        return Err(OUTSIDE.to_string());
    }
    let dest_dir = fs::canonicalize(dest_dir).map_err(|e| e.to_string())?;
    if !dest_dir.starts_with(&root) {
        return Err(OUTSIDE.to_string());
    }

    for _ in 0..MAX_ATTEMPTS {
        let candidate = unique_dest(dest);
        match rename_no_replace(src, &candidate) {
            Ok(()) => return Ok(candidate),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    Err(NO_FREE_NAME.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn planned(id: u32, src: &Path, root: &Path, name: &str, category: Category) -> PlannedMove {
        PlannedMove {
            id,
            src: src.to_string_lossy().into_owned(),
            dest: root
                .join(folder_name(category))
                .join(name)
                .to_string_lossy()
                .into_owned(),
            category,
        }
    }

    #[test]
    fn move_creates_folder_moves_file_and_reports_final_dest() {
        let root = tempdir().unwrap();
        let src = root.path().join("a.jpg");
        fs::write(&src, b"foto").unwrap();
        let m = planned(1, &src, root.path(), "a.jpg", Category::Fotos);

        let created = ensure_folders(root.path(), &[Category::Fotos]).unwrap();
        assert_eq!(created, vec![root.path().join("Foto's")]);
        let outcome = move_one(&m);

        assert!(outcome.ok, "{:?}", outcome.error);
        assert_eq!(outcome.id, 1);
        assert_eq!(outcome.error, None);
        let dest = PathBuf::from(outcome.dest.unwrap());
        assert_eq!(dest, root.path().join("Foto's").join("a.jpg"));
        assert!(dest.exists());
        assert!(!src.exists());
        assert_eq!(fs::read(&dest).unwrap(), b"foto");
    }

    #[test]
    fn move_resolves_collision_without_overwriting() {
        let root = tempdir().unwrap();
        ensure_folders(root.path(), &[Category::Fotos]).unwrap();
        let existing = root.path().join("Foto's").join("a.jpg");
        fs::write(&existing, b"oorspronkelijk").unwrap();
        let src = root.path().join("a.jpg");
        fs::write(&src, b"nieuw").unwrap();
        let m = planned(7, &src, root.path(), "a.jpg", Category::Fotos);

        let outcome = move_one(&m);

        assert!(outcome.ok, "{:?}", outcome.error);
        let dest = outcome.dest.unwrap();
        assert!(dest.ends_with(" (2).jpg"), "{dest}");
        assert_eq!(fs::read(&existing).unwrap(), b"oorspronkelijk");
        assert_eq!(fs::read(&dest).unwrap(), b"nieuw");
        assert!(!src.exists());
    }

    #[cfg(windows)]
    #[test]
    fn move_locked_file_reports_error_and_leaves_source() {
        use std::fs::OpenOptions;
        use std::os::windows::fs::OpenOptionsExt;

        let root = tempdir().unwrap();
        ensure_folders(root.path(), &[Category::Fotos]).unwrap();
        let src = root.path().join("vast.jpg");
        fs::write(&src, b"in gebruik").unwrap();
        let m = planned(3, &src, root.path(), "vast.jpg", Category::Fotos);

        let lock = OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&src)
            .unwrap();
        let outcome = move_one(&m);
        drop(lock);

        assert!(!outcome.ok);
        assert_eq!(outcome.dest, None);
        assert!(!outcome.error.unwrap().is_empty());
        assert!(src.exists());
        assert!(!root.path().join("Foto's").join("vast.jpg").exists());
    }

    #[cfg(windows)]
    #[test]
    fn rename_no_replace_refuses_existing_target_and_leaves_both_untouched() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("nieuw.txt");
        let target = dir.path().join("doel.txt");
        fs::write(&src, b"nieuw").unwrap();
        fs::write(&target, b"oorspronkelijk").unwrap();

        let err = rename_no_replace(&src, &target).unwrap_err();

        assert_eq!(err.kind(), ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&target).unwrap(), b"oorspronkelijk");
        assert_eq!(fs::read(&src).unwrap(), b"nieuw");
    }

    #[test]
    fn rename_no_replace_moves_to_free_target() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("nieuw.txt");
        let target = dir.path().join("doel.txt");
        fs::write(&src, b"nieuw").unwrap();

        rename_no_replace(&src, &target).unwrap();

        assert_eq!(fs::read(&target).unwrap(), b"nieuw");
        assert!(!src.exists());
    }

    #[test]
    fn move_refuses_paths_outside_root() {
        let root = tempdir().unwrap();
        let elders = tempdir().unwrap();
        let src = elders.path().join("geheim.pdf");
        fs::write(&src, b"niet aanraken").unwrap();
        ensure_folders(root.path(), &[Category::Pdfs]).unwrap();
        let m = planned(9, &src, root.path(), "geheim.pdf", Category::Pdfs);

        let outcome = move_one(&m);

        assert!(!outcome.ok);
        assert_eq!(outcome.dest, None);
        assert!(outcome.error.unwrap().contains("buiten de map"));
        assert!(src.exists());
        assert!(!root.path().join("PDF's").join("geheim.pdf").exists());
    }

    #[test]
    fn move_reports_error_when_destination_folder_is_missing() {
        let root = tempdir().unwrap();
        let src = root.path().join("a.jpg");
        fs::write(&src, b"foto").unwrap();
        let m = planned(2, &src, root.path(), "a.jpg", Category::Fotos);

        let outcome = move_one(&m);

        assert!(!outcome.ok);
        assert!(outcome.error.is_some());
        assert!(src.exists());
    }

    #[test]
    fn ensure_folders_returns_only_new_ones() {
        let root = tempdir().unwrap();
        fs::create_dir(root.path().join("Foto's")).unwrap();

        let created = ensure_folders(
            root.path(),
            &[Category::Fotos, Category::Pdfs, Category::Pdfs],
        )
        .unwrap();

        assert_eq!(created, vec![root.path().join("PDF's")]);
        assert!(root.path().join("PDF's").is_dir());
    }

    #[test]
    fn move_many_reports_each_file_and_continues_after_failure() {
        let root = tempdir().unwrap();
        ensure_folders(root.path(), &[Category::Fotos]).unwrap();
        let ok_src = root.path().join("a.jpg");
        fs::write(&ok_src, b"foto").unwrap();
        let missing = root.path().join("weg.jpg");
        let moves = vec![
            planned(1, &missing, root.path(), "weg.jpg", Category::Fotos),
            planned(2, &ok_src, root.path(), "a.jpg", Category::Fotos),
        ];

        let outcomes = move_many(&moves);

        assert_eq!(outcomes.len(), 2);
        assert_eq!((outcomes[0].id, outcomes[0].ok), (1, false));
        assert_eq!((outcomes[1].id, outcomes[1].ok), (2, true));
        assert!(root.path().join("Foto's").join("a.jpg").exists());
    }
}
