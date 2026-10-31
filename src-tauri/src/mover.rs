use crate::categories::{Category, folder_name};
use crate::planner::{PlannedMove, unique_dest};
use serde::Serialize;
use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

const OUTSIDE: &str = "Het bestand ligt buiten de map";
const INVALID_DEST: &str = "Ongeldige bestemming";

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

/// Verplaatst één bestand met één `fs::rename` naar een vrije naam. Overschrijft nooit, volgt
/// geen symlinks en blijft binnen de gekozen map (die volgt uit `dest`: `root/<map>/<naam>`).
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

    let final_dest = unique_dest(dest);
    fs::rename(src, &final_dest).map_err(|e| e.to_string())?;
    Ok(final_dest)
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
