use crate::categories::{self, Category};
use crate::paths;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct FileEntry {
    pub id: u32,
    pub name: String,
    pub path: String,
    pub size: u64,
    pub modified_ms: u64,
    pub category: Category,
    pub is_duplicate: bool,
    pub duplicate_of: Option<u32>,
    pub placeholder: bool,
}

#[derive(Clone, Serialize, Debug)]
pub struct ScanResult {
    pub root: String,
    pub files: Vec<FileEntry>,
    pub existing_category_folders: Vec<Category>,
    pub skipped: u32,
}

#[derive(Debug, Serialize)]
pub enum ScanError {
    NotADirectory,
    Forbidden,
    Io(String),
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScanError::NotADirectory => write!(f, "dit is geen map"),
            ScanError::Forbidden => write!(f, "deze map mag niet opgeruimd worden"),
            ScanError::Io(msg) => write!(f, "de map kon niet gelezen worden: {msg}"),
        }
    }
}

const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;
const FILE_ATTRIBUTE_OFFLINE: u32 = 0x1000;
const FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS: u32 = 0x40_0000;

const SKIPPED_FILE_NAMES: &[&str] = &["desktop.ini", "thumbs.db"];
const SKIPPED_SUFFIXES: &[&str] = &[".tmp", ".crdownload", ".part", ".partial"];

/// Lijst de losse bestanden van één map (geen submappen, geen symlinks). Duplicaten zijn
/// hier nog niet gemarkeerd. Verborgen en tijdelijke bestanden en symlinks tellen mee in
/// `skipped`; submappen niet.
pub fn scan_directory(root: &Path) -> Result<ScanResult, ScanError> {
    if paths::is_forbidden_root(root) {
        return Err(ScanError::Forbidden);
    }
    if !root.is_dir() {
        return Err(ScanError::NotADirectory);
    }

    let mut files = Vec::new();
    let mut skipped = 0;
    for entry in fs::read_dir(root).map_err(io_error)? {
        let entry = entry.map_err(io_error)?;
        // Een bestand dat net verdwenen is of niet leesbaar is, tellen we als overgeslagen.
        let Ok(metadata) = fs::symlink_metadata(entry.path()) else {
            skipped += 1;
            continue;
        };
        let file_type = metadata.file_type();
        if file_type.is_symlink() {
            skipped += 1;
            continue;
        }
        if file_type.is_dir() {
            continue;
        }
        let attrs = file_attributes(&metadata);
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            skipped += 1;
            continue;
        };
        if is_skipped_name(&name) || attrs & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM) != 0 {
            skipped += 1;
            continue;
        }
        files.push(FileEntry {
            id: 0,
            category: categories::categorize(&name),
            name,
            path: entry.path().to_string_lossy().into_owned(),
            size: metadata.len(),
            modified_ms: modified_ms(&metadata),
            is_duplicate: false,
            duplicate_of: None,
            placeholder: is_placeholder(attrs),
        });
    }

    files.sort_by_cached_key(|f| (f.name.to_lowercase(), f.name.clone()));
    for (index, file) in files.iter_mut().enumerate() {
        file.id = index as u32;
    }

    let existing_category_folders = categories::all()
        .iter()
        .copied()
        .filter(|&category| root.join(categories::folder_name(category)).is_dir())
        .collect();

    Ok(ScanResult {
        root: root.to_string_lossy().into_owned(),
        files,
        existing_category_folders,
        skipped,
    })
}

/// Bestanden die we nooit aanraken: verborgen (punt vooraan), `desktop.ini`, `Thumbs.db`
/// en onafgewerkte downloads. Hoofdletterongevoelig.
pub fn is_skipped_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.starts_with('.')
        || SKIPPED_FILE_NAMES.contains(&lower.as_str())
        || SKIPPED_SUFFIXES
            .iter()
            .any(|suffix| lower.ends_with(suffix))
}

/// Bestand staat nog in de cloud (OneDrive e.a.) en is niet volledig op deze pc.
pub fn is_placeholder(attrs: u32) -> bool {
    attrs & (FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS | FILE_ATTRIBUTE_OFFLINE) != 0
}

fn io_error(err: std::io::Error) -> ScanError {
    ScanError::Io(err.to_string())
}

fn modified_ms(metadata: &fs::Metadata) -> u64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .and_then(|elapsed| u64::try_from(elapsed.as_millis()).ok())
        .unwrap_or(0)
}

#[cfg(windows)]
fn file_attributes(metadata: &fs::Metadata) -> u32 {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes()
}

#[cfg(not(windows))]
fn file_attributes(_metadata: &fs::Metadata) -> u32 {
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn touch(dir: &Path, name: &str) {
        fs::write(dir.join(name), b"x").unwrap();
    }

    #[test]
    fn scan_lists_only_loose_files_and_skips_hidden_temp_and_dirs() {
        let dir = tempdir().unwrap();
        for name in [
            "a.jpg",
            "B.PDF",
            ".env",
            "download.crdownload",
            "desktop.ini",
        ] {
            touch(dir.path(), name);
        }
        fs::create_dir(dir.path().join("sub")).unwrap();
        touch(&dir.path().join("sub"), "c.txt");

        let r = scan_directory(dir.path()).unwrap();
        let names: Vec<_> = r.files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["a.jpg", "B.PDF"]);
        assert_eq!(r.skipped, 3);
        assert_eq!(r.files[0].category, Category::Fotos);
        assert_eq!(r.files[1].category, Category::Pdfs);
        assert!(r.files[0].id == 0 && r.files[1].id == 1);
    }

    #[test]
    fn scan_fills_entry_details() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("notes.txt"), b"hello").unwrap();

        let r = scan_directory(dir.path()).unwrap();
        assert_eq!(r.root, dir.path().to_string_lossy());
        let f = &r.files[0];
        assert_eq!(f.size, 5);
        assert!(f.modified_ms > 0);
        assert_eq!(f.category, Category::Documenten);
        assert_eq!(Path::new(&f.path), dir.path().join("notes.txt"));
        assert!(!f.is_duplicate && f.duplicate_of.is_none() && !f.placeholder);
    }

    #[test]
    fn scan_sorts_case_insensitively_with_original_name_as_tiebreak() {
        let dir = tempdir().unwrap();
        for name in ["b.txt", "A.txt", "a.txt0", "C.txt"] {
            touch(dir.path(), name);
        }
        let r = scan_directory(dir.path()).unwrap();
        let names: Vec<_> = r.files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["A.txt", "a.txt0", "b.txt", "C.txt"]);
        let ids: Vec<_> = r.files.iter().map(|f| f.id).collect();
        assert_eq!(ids, vec![0, 1, 2, 3]);
    }

    #[test]
    fn scan_reports_existing_category_folders() {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join("Foto's")).unwrap();
        fs::create_dir(dir.path().join("Willekeurig")).unwrap();
        fs::create_dir(dir.path().join("Audio")).unwrap();
        touch(dir.path(), "PDF's"); // een bestand telt niet als map

        let r = scan_directory(dir.path()).unwrap();
        assert_eq!(
            r.existing_category_folders,
            vec![Category::Fotos, Category::Audio]
        );
    }

    #[test]
    fn scan_handles_non_ascii_root() {
        let base = tempdir().unwrap();
        let root = base.path().join("Téléchargements é");
        fs::create_dir(&root).unwrap();
        touch(&root, "café.png");

        let r = scan_directory(&root).unwrap();
        assert_eq!(r.files.len(), 1);
        assert_eq!(r.files[0].name, "café.png");
        assert!(r.files[0].path.starts_with(root.to_string_lossy().as_ref()));
    }

    #[test]
    fn scan_rejects_a_file_a_missing_path_and_a_forbidden_root() {
        let dir = tempdir().unwrap();
        touch(dir.path(), "a.txt");
        assert!(matches!(
            scan_directory(&dir.path().join("a.txt")),
            Err(ScanError::NotADirectory)
        ));
        assert!(matches!(
            scan_directory(&dir.path().join("bestaat-niet")),
            Err(ScanError::NotADirectory)
        ));
        #[cfg(windows)]
        assert!(matches!(
            scan_directory(Path::new("C:\\")),
            Err(ScanError::Forbidden)
        ));
    }

    #[test]
    fn scan_skips_and_counts_symlinks() {
        let dir = tempdir().unwrap();
        touch(dir.path(), "real.txt");
        if try_symlink_file(&dir.path().join("real.txt"), &dir.path().join("link.txt")).is_err() {
            eprintln!("symlinks niet toegestaan in deze omgeving, test overgeslagen");
            return;
        }
        let r = scan_directory(dir.path()).unwrap();
        let names: Vec<_> = r.files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["real.txt"]);
        assert_eq!(r.skipped, 1);
    }

    #[cfg(windows)]
    #[test]
    fn scan_skips_files_with_the_windows_hidden_attribute() {
        let dir = tempdir().unwrap();
        touch(dir.path(), "zichtbaar.txt");
        touch(dir.path(), "verborgen.txt");
        let status = std::process::Command::new("attrib")
            .arg("+h")
            .arg(dir.path().join("verborgen.txt"))
            .status()
            .unwrap();
        assert!(status.success());

        let r = scan_directory(dir.path()).unwrap();
        let names: Vec<_> = r.files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["zichtbaar.txt"]);
        assert_eq!(r.skipped, 1);
    }

    #[cfg(windows)]
    fn try_symlink_file(src: &Path, dst: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_file(src, dst)
    }

    #[cfg(unix)]
    fn try_symlink_file(src: &Path, dst: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(src, dst)
    }

    #[test]
    fn skipped_names() {
        assert!(is_skipped_name(".env"));
        assert!(is_skipped_name("Thumbs.db"));
        assert!(is_skipped_name("x.crdownload"));
        assert!(!is_skipped_name("a.b.c.jpg"));
        assert!(is_skipped_name("desktop.ini"));
        assert!(is_skipped_name("DESKTOP.INI"));
        assert!(is_skipped_name("thumbs.db"));
        assert!(is_skipped_name("film.part"));
        assert!(is_skipped_name("film.PARTIAL"));
        assert!(is_skipped_name("cache.tmp"));
        assert!(!is_skipped_name("tmp"));
        assert!(!is_skipped_name("parttime.pdf"));
    }

    #[test]
    fn placeholder_attrs() {
        assert!(is_placeholder(0x400000));
        assert!(is_placeholder(0x1000));
        assert!(!is_placeholder(0x20));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn mac_scan_skips_hidden_downloads_cloud_names_and_packages() {
        let dir = tempdir().unwrap();
        for name in ["local.txt", "hidden.txt", ".env", "file.icloud", "file.download", "file.part"] {
            touch(dir.path(), name);
        }
        fs::create_dir(dir.path().join("App.app")).unwrap();
        touch(&dir.path().join("App.app"), "inside.txt");
        assert!(std::process::Command::new("chflags").arg("hidden").arg(dir.path().join("hidden.txt")).status().unwrap().success());
        let result = scan_directory(dir.path()).unwrap();
        assert_eq!(result.files.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["local.txt"]);
        assert_eq!(result.skipped, 5);
    }

    #[cfg(unix)]
    #[test]
    fn scan_skips_special_files_without_opening_them() {
        let dir = tempdir().unwrap();
        let socket = std::os::unix::net::UnixListener::bind(dir.path().join("socket")).unwrap();
        let result = scan_directory(dir.path()).unwrap();
        assert!(result.files.is_empty());
        assert_eq!(result.skipped, 1);
        drop(socket);
    }

    #[cfg(windows)]
    #[test]
    fn scan_skips_offline_files_before_processing() {
        let dir = tempdir().unwrap();
        touch(dir.path(), "offline.txt");
        assert!(std::process::Command::new("attrib").arg("+o").arg(dir.path().join("offline.txt")).status().unwrap().success());
        let result = scan_directory(dir.path()).unwrap();
        assert!(result.files.is_empty());
        assert_eq!(result.skipped, 1);
    }
}
