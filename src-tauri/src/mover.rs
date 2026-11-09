use crate::categories::{Category, folder_name};
use crate::planner::{PlannedMove, unique_dest};
use serde::Serialize;
use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

pub(crate) const OUTSIDE: &str = "Het bestand ligt buiten de map";
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
/// naar een andere schijf en geeft `AlreadyExists` als het doel er is. De paden gaan verbatim
/// mee, zodat ook paden boven MAX_PATH (260 tekens) werken zoals bij `std::fs::rename`.
#[cfg(windows)]
fn rename_no_replace(src: &Path, dest: &Path) -> io::Result<()> {
    use windows_sys::Win32::Storage::FileSystem::MoveFileExW;

    let (src, dest) = (wide_verbatim(src), wide_verbatim(dest));
    // SAFETY: beide buffers zijn met 0 afgesloten en leven langer dan de aanroep.
    let moved = unsafe { MoveFileExW(src.as_ptr(), dest.as_ptr(), 0) };
    if moved == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// Het pad als met 0 afgesloten UTF-16 in verbatim vorm: `C:\map` wordt `\\?\C:\map` en
/// `\\server\share\map` wordt `\\?\UNC\server\share\map`. Een absoluut pad wordt eerst genormaliseerd
/// (schuine strepen, `.` en `..`), want een verbatim pad wordt niet meer genormaliseerd. Paden die
/// al verbatim zijn en relatieve paden blijven zoals ze zijn.
#[cfg(windows)]
fn wide_verbatim(path: &Path) -> Vec<u16> {
    use std::iter::once;
    use std::os::windows::ffi::OsStrExt;
    use std::path::{Component, Prefix};

    let path = if path.is_absolute() {
        std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
    } else {
        path.to_path_buf()
    };
    let (prefix, skip) = match path.components().next() {
        Some(Component::Prefix(p)) if path.is_absolute() => match p.kind() {
            Prefix::Disk(_) => (r"\\?\", 0),
            Prefix::UNC(..) => (r"\\?\UNC\", 2),
            _ => ("", 0),
        },
        _ => ("", 0),
    };
    prefix
        .encode_utf16()
        .chain(path.as_os_str().encode_wide().skip(skip))
        .chain(once(0))
        .collect()
}

/// Buiten Windows bestaat er in `std` geen rename zonder vervangen; Netjes draait enkel op Windows,
/// dit houdt de bibliotheek compileerbaar.
#[cfg(not(windows))]
fn rename_no_replace(src: &Path, dest: &Path) -> io::Result<()> {
    fs::rename(src, dest)
}

/// Verplaatst `src` naar `dest`, of naar de eerstvolgende vrije naam als `dest` al bezet is, en
/// geeft het uiteindelijke pad terug. Nooit vervangen: zie `rename_no_replace`. Controleert niet
/// of de paden binnen een bepaalde map blijven; dat doet de aanroeper.
pub fn move_path_no_replace(src: &Path, dest: &Path) -> io::Result<PathBuf> {
    for _ in 0..MAX_ATTEMPTS {
        let candidate = unique_dest(dest);
        match rename_no_replace(src, &candidate) {
            Ok(()) => return Ok(candidate),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::new(ErrorKind::AlreadyExists, NO_FREE_NAME))
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

    move_path_no_replace(src, dest).map_err(|e| e.to_string())
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

    #[cfg(windows)]
    #[test]
    fn move_works_when_the_path_is_longer_than_260_characters() {
        let tmp = tempdir().unwrap();
        let root = tmp.path().join("a".repeat(200));
        fs::create_dir_all(&root).unwrap();
        ensure_folders(&root, &[Category::Fotos]).unwrap();
        let name = format!("{}.jpg", "b".repeat(50));
        let src = root.join(&name);
        fs::write(&src, b"lang pad").unwrap();
        let m = planned(5, &src, &root, &name, Category::Fotos);
        assert!(m.dest.len() > 260, "{}", m.dest.len());

        let outcome = move_one(&m);

        assert!(outcome.ok, "{:?}", outcome.error);
        let dest = PathBuf::from(outcome.dest.unwrap());
        assert_eq!(dest, root.join("Foto's").join(&name));
        assert_eq!(fs::read(&dest).unwrap(), b"lang pad");
        assert!(!src.exists());
    }

    #[cfg(windows)]
    #[test]
    fn verbatim_prefixes_absolute_paths_only() {
        let text = |p: &str| {
            let wide = wide_verbatim(Path::new(p));
            assert_eq!(wide.last(), Some(&0));
            String::from_utf16(&wide[..wide.len() - 1]).unwrap()
        };
        assert_eq!(text(r"C:\Users\x\a.jpg"), r"\\?\C:\Users\x\a.jpg");
        assert_eq!(text("C:/Users/x/a.jpg"), r"\\?\C:\Users\x\a.jpg");
        assert_eq!(
            text(r"\\server\share\map\a.jpg"),
            r"\\?\UNC\server\share\map\a.jpg"
        );
        assert_eq!(text(r"\\?\C:\Users\a.jpg"), r"\\?\C:\Users\a.jpg");
        assert_eq!(text(r"map\a.jpg"), r"map\a.jpg");
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

    #[test]
    fn move_path_no_replace_uses_free_name_and_never_overwrites() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("nieuw.txt");
        let target = dir.path().join("doel.txt");
        fs::write(&src, b"nieuw").unwrap();

        let first = move_path_no_replace(&src, &target).unwrap();
        assert_eq!(first, target);
        assert_eq!(fs::read(&target).unwrap(), b"nieuw");
        assert!(!src.exists());

        fs::write(&src, b"tweede").unwrap();
        let second = move_path_no_replace(&src, &target).unwrap();
        assert_eq!(second, dir.path().join("doel (2).txt"));
        assert_eq!(fs::read(&target).unwrap(), b"nieuw");
        assert_eq!(fs::read(&second).unwrap(), b"tweede");
        assert!(!src.exists());
    }

    #[test]
    fn move_path_no_replace_reports_missing_source() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("doel.txt");

        let err = move_path_no_replace(&dir.path().join("weg.txt"), &target).unwrap_err();

        assert_eq!(err.kind(), ErrorKind::NotFound);
        assert!(!target.exists());
    }

    #[test]
    fn move_refuses_directory_sources() {
        let root = tempdir().unwrap();
        let src = root.path().join("pakket.app");
        fs::create_dir(&src).unwrap();
        ensure_folders(root.path(), &[Category::Fotos]).unwrap();
        let outcome = move_one(&planned(1, &src, root.path(), "pakket.app", Category::Fotos));
        assert!(!outcome.ok);
        assert!(src.is_dir());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn rename_no_replace_preserves_native_path_bytes_and_system_errors() {
        use std::os::unix::ffi::OsStringExt;
        let root = tempdir().unwrap();
        let src = root.path().join(std::ffi::OsString::from_vec(vec![b'a', 0xff]));
        let dest = root.path().join("doel");
        fs::write(&src, b"native").unwrap();
        rename_no_replace(&src, &dest).unwrap();
        assert_eq!(fs::read(&dest).unwrap(), b"native");
        assert_eq!(rename_no_replace(&src, &root.path().join("weg")).unwrap_err().kind(), ErrorKind::NotFound);
    }

    #[cfg(unix)]
    #[test]
    fn move_refuses_symlink_source_and_destination_directory() {
        use std::os::unix::fs::symlink;
        let root = tempdir().unwrap();
        let outside = tempdir().unwrap();
        let real = outside.path().join("real.jpg");
        fs::write(&real, b"blijft").unwrap();
        let src = root.path().join("link.jpg");
        symlink(&real, &src).unwrap();
        ensure_folders(root.path(), &[Category::Fotos]).unwrap();
        assert!(!move_one(&planned(1, &src, root.path(), "link.jpg", Category::Fotos)).ok);
        fs::remove_file(&src).unwrap();
        fs::write(&src, b"lokaal").unwrap();
        fs::remove_dir(root.path().join("Foto's")).unwrap();
        symlink(outside.path(), root.path().join("Foto's")).unwrap();
        assert!(ensure_folders(root.path(), &[Category::Fotos]).is_err());
        assert!(!move_one(&planned(1, &src, root.path(), "link.jpg", Category::Fotos)).ok);
        assert_eq!(fs::read(src).unwrap(), b"lokaal");
        assert_eq!(fs::read(real).unwrap(), b"blijft");
    }
}
