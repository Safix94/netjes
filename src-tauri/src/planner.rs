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

/// Geeft `dest` terug als daar niets staat. Anders " (2)", " (3)", ... vóór de extensie:
/// `versie 1.2 final.pdf` wordt `versie 1.2 final (2).pdf`, `archief.tar.gz` wordt
/// `archief (2).tar.gz` en een naam zonder extensie krijgt de teller achteraan.
pub fn unique_dest(dest: &Path) -> PathBuf {
    if is_free(dest) {
        return dest.to_path_buf();
    }
    let Some(name) = dest.file_name().map(|n| n.to_string_lossy()) else {
        return dest.to_path_buf();
    };
    let (stem, extension) = split_extension(&name);
    (2u32..)
        .map(|n| dest.with_file_name(format!("{stem} ({n}){extension}")))
        .find(|candidate| is_free(candidate))
        .unwrap_or_else(|| dest.to_path_buf())
}

/// Splitst een bestandsnaam in stam en extensie (met de punt). De extensie begint bij de laatste
/// punt, behalve een punt vooraan (`.env` heeft er geen) en een "extensie" met een spatie
/// (`Notities. Concept` heeft er geen). Staat er vlak voor die punt `.tar`, dan hoort dat erbij:
/// `x.tar.gz`, `x.tar.xz`.
fn split_extension(name: &str) -> (&str, &str) {
    let Some(dot) = name.rfind('.').filter(|&i| i >= 1) else {
        return (name, "");
    };
    if name[dot..].contains(' ') {
        return (name, "");
    }
    let before = &name[..dot];
    let split = match before.len().checked_sub(4) {
        Some(tar)
            if tar >= 1
                && before
                    .get(tar..)
                    .is_some_and(|t| t.eq_ignore_ascii_case(".tar")) =>
        {
            tar
        }
        _ => dot,
    };
    name.split_at(split)
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

    fn counted(existing: &str) -> PathBuf {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join(existing), b"1").unwrap();
        let unique = unique_dest(&dir.path().join(existing));
        unique.strip_prefix(dir.path()).unwrap().to_path_buf()
    }

    #[test]
    fn unique_dest_counts_before_the_last_dot() {
        assert_eq!(
            counted("versie 1.2 final.pdf"),
            Path::new("versie 1.2 final (2).pdf")
        );
        assert_eq!(
            counted("IMG 2026.10.31.jpg"),
            Path::new("IMG 2026.10.31 (2).jpg")
        );
        assert_eq!(counted("foto.jpg"), Path::new("foto (2).jpg"));
    }

    #[test]
    fn unique_dest_keeps_tar_in_the_extension() {
        assert_eq!(counted("x.tar.gz"), Path::new("x (2).tar.gz"));
        assert_eq!(counted("x.tar.bz2"), Path::new("x (2).tar.bz2"));
        assert_eq!(counted("x.tar.xz"), Path::new("x (2).tar.xz"));
        assert_eq!(counted("x.tar.zst"), Path::new("x (2).tar.zst"));
        assert_eq!(
            counted("Back-up 1.2.TAR.GZ"),
            Path::new("Back-up 1.2 (2).TAR.GZ")
        );
        assert_eq!(counted("x.tar"), Path::new("x (2).tar"));
    }

    #[test]
    fn unique_dest_treats_extension_with_space_as_no_extension() {
        assert_eq!(
            counted("Notities. Concept"),
            Path::new("Notities. Concept (2)")
        );
    }

    #[test]
    fn unique_dest_leading_dot_is_not_an_extension() {
        assert_eq!(counted(".env"), Path::new(".env (2)"));
        assert_eq!(counted(".env.local"), Path::new(".env (2).local"));
        assert_eq!(counted(".tar.gz"), Path::new(".tar (2).gz"));
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
