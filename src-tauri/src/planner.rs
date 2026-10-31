use crate::categories::{self, Category};
use crate::scan::FileEntry;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// Eén geplande verplaatsing: `dest` is `root/<categoriemap>/<naam>`, nog zonder conflictoplossing.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct PlannedMove {
    pub id: u32,
    pub src: String,
    pub dest: String,
    pub category: Category,
}

/// Plant elke bestandsvermelding naar `root/<categoriemap>/<naam>`. Een duplicaat heeft al
/// de categorie `Duplicaten`, dus die volgt zonder uitzondering dezelfde regel.
pub fn plan_moves(root: &Path, files: &[FileEntry]) -> Vec<PlannedMove> {
    files
        .iter()
        .map(|f| PlannedMove {
            id: f.id,
            src: f.path.clone(),
            dest: root
                .join(categories::folder_name(f.category))
                .join(&f.name)
                .to_string_lossy()
                .into_owned(),
            category: f.category,
        })
        .collect()
}

/// Geeft `dest` terug als daar niets staat. Anders " (2)", " (3)", ... vóór de volledige
/// extensieketen: `archief.tar.gz` wordt `archief (2).tar.gz`, een naam zonder punt krijgt de
/// teller achteraan. Een naam die met een punt begint (`.env`) heeft geen extensie.
pub fn unique_dest(dest: &Path) -> PathBuf {
    if is_free(dest) {
        return dest.to_path_buf();
    }
    let Some(name) = dest.file_name().map(|n| n.to_string_lossy()) else {
        return dest.to_path_buf();
    };
    let split = name
        .char_indices()
        .skip(1)
        .find(|&(_, c)| c == '.')
        .map_or(name.len(), |(i, _)| i);
    let (stem, extensions) = name.split_at(split);
    (2u32..)
        .map(|n| dest.with_file_name(format!("{stem} ({n}){extensions}")))
        .find(|candidate| is_free(candidate))
        .unwrap_or_else(|| dest.to_path_buf())
}

/// Vrij is enkel wat zeker niet bestaat. Een dode symlink of een onleesbare plek telt als bezet.
fn is_free(path: &Path) -> bool {
    matches!(fs::symlink_metadata(path), Err(e) if e.kind() == ErrorKind::NotFound)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn entry(id: u32, root: &Path, name: &str, category: Category) -> FileEntry {
        FileEntry {
            id,
            name: name.to_string(),
            path: root.join(name).to_string_lossy().into_owned(),
            size: 1,
            modified_ms: 0,
            category,
            is_duplicate: category == Category::Duplicaten,
            duplicate_of: None,
            placeholder: false,
        }
    }

    #[test]
    fn unique_dest_adds_counter_before_full_extension() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("x.tar.gz"), b"1").unwrap();
        fs::write(dir.path().join("x (2).tar.gz"), b"2").unwrap();
        let unique = unique_dest(&dir.path().join("x.tar.gz"));
        assert_eq!(unique, dir.path().join("x (3).tar.gz"));
    }

    #[test]
    fn unique_dest_returns_dest_itself_when_free() {
        let dir = tempdir().unwrap();
        let dest = dir.path().join("vrij.pdf");
        assert_eq!(unique_dest(&dest), dest);
    }

    #[test]
    fn unique_dest_appends_counter_to_name_without_dot() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("README"), b"1").unwrap();
        assert_eq!(
            unique_dest(&dir.path().join("README")),
            dir.path().join("README (2)")
        );
    }

    #[test]
    fn unique_dest_does_not_panic_on_leading_dot() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join(".env"), b"1").unwrap();
        assert_eq!(
            unique_dest(&dir.path().join(".env")),
            dir.path().join(".env (2)")
        );
    }

    #[test]
    fn plan_puts_files_in_category_folders() {
        let root = Path::new("C:\\Downloads");
        let files = vec![
            entry(1, root, "a.jpg", Category::Fotos),
            entry(2, root, "a (1).jpg", Category::Duplicaten),
            entry(3, root, "factuur.pdf", Category::Pdfs),
        ];
        let plan = plan_moves(root, &files);
        assert_eq!(plan.len(), 3);
        assert_eq!(plan[0].id, 1);
        assert_eq!(Path::new(&plan[0].src), root.join("a.jpg"));
        assert_eq!(Path::new(&plan[0].dest), root.join("Foto's").join("a.jpg"));
        assert_eq!(plan[0].category, Category::Fotos);
        assert_eq!(
            Path::new(&plan[1].dest),
            root.join("Duplicaten").join("a (1).jpg")
        );
        assert_eq!(plan[1].category, Category::Duplicaten);
        assert_eq!(
            Path::new(&plan[2].dest),
            root.join("PDF's").join("factuur.pdf")
        );
    }
}
