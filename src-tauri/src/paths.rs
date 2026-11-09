use std::path::{Path, PathBuf};

/// Standaardlocaties van de systeemmappen, voor als de omgevingsvariabelen ontbreken.
#[cfg(windows)]
const DEFAULT_PROTECTED_TREES: &[&str] = &[
    "c:\\windows",
    "c:\\program files",
    "c:\\program files (x86)",
];

/// Omgevingsvariabelen die een beschermde mapboom aanwijzen.
#[cfg(windows)]
const PROTECTED_TREE_VARS: &[&str] = &[
    "WINDIR",
    "SystemRoot",
    "ProgramFiles",
    "ProgramFiles(x86)",
    "ProgramW6432",
];

/// `%LOCALAPPDATA%\Netjes`, aangemaakt als die nog niet bestaat. Zonder lokale
/// gegevensmap valt dit terug op de tijdelijke map van het systeem.
pub fn app_data_dir() -> PathBuf {
    let dir = dirs::data_local_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("Netjes");
    // Mislukt dit, dan krijgt de aanroeper dezelfde fout bij het eerste schrijven.
    let _ = std::fs::create_dir_all(&dir);
    dir
}

pub fn downloads_dir() -> Option<PathBuf> {
    dirs::download_dir()
}

/// Mappen die Netjes nooit mag opruimen: schijfwortels, `%WINDIR%` en `Program Files`
/// (met alles eronder) en de gebruikersmap zelf (niet de submappen, zoals Downloads).
/// Het pad hoeft niet te bestaan; vergelijking is hoofdletterongevoelig.
#[cfg(windows)]
pub fn is_forbidden_root(p: &Path) -> bool {
    // Controleer ook het echte doel van junctions/symlinks, zonder bestaande Windows-hashes te wijzigen.
    forbidden_windows_path(p)
        || std::fs::canonicalize(p).is_ok_and(|real| forbidden_windows_path(&real))
}

#[cfg(windows)]
fn forbidden_windows_path(p: &Path) -> bool {
    let path = normalize(&p.to_string_lossy());
    if is_drive_root(&path) {
        return true;
    }
    if dirs::home_dir().is_some_and(|home| normalize(&home.to_string_lossy()) == path) {
        return true;
    }
    let from_env = PROTECTED_TREE_VARS
        .iter()
        .filter_map(|var| std::env::var(var).ok())
        .map(|value| normalize(&value));
    let defaults = DEFAULT_PROTECTED_TREES
        .iter()
        .map(|tree| (*tree).to_string());
    from_env
        .chain(defaults)
        .filter(|tree| !tree.is_empty())
        .any(|tree| path == tree || path.starts_with(&format!("{tree}\\")))
}

/// Kleine letters, `\` als scheidingsteken, zonder `\\?\`-voorvoegsel of slotscheidingstekens.
#[cfg(windows)]
fn normalize(path: &str) -> String {
    let lower = path.to_lowercase().replace('/', "\\");
    let without_prefix = lower.strip_prefix("\\\\?\\").unwrap_or(&lower);
    without_prefix.trim_end_matches('\\').to_string()
}

/// `c:` (het genormaliseerde `C:\`): een letter, een dubbele punt en verder niets.
#[cfg(windows)]
fn is_drive_root(normalized: &str) -> bool {
    let mut chars = normalized.chars();
    matches!(
        (chars.next(), chars.next(), chars.next()),
        (Some(letter), Some(':'), None) if letter.is_ascii_alphabetic()
    )
}

#[cfg(target_os = "macos")]
pub fn is_forbidden_root(path: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;

    let Ok(absolute) = std::path::absolute(path) else {
        return true;
    };
    // Bestaande voorouders oplossen beschermt ook een nog niet bestaande Library-submap.
    let real = absolute
        .ancestors()
        .find_map(|ancestor| {
            std::fs::canonicalize(ancestor)
                .ok()
                .map(|real| real.join(absolute.strip_prefix(ancestor).unwrap_or(Path::new(""))))
        })
        .unwrap_or(absolute);
    if real.parent().is_none() {
        return true;
    }
    if let (Ok(meta), Some(parent)) = (std::fs::metadata(&real), real.parent()) {
        if std::fs::metadata(parent).is_ok_and(|parent| meta.dev() != parent.dev()) {
            return true; // Een mountpoint, ook buiten /Volumes.
        }
    }
    let same_directory = |a: &Path, b: &Path| match (std::fs::metadata(a), std::fs::metadata(b)) {
        (Ok(a), Ok(b)) => a.dev() == b.dev() && a.ino() == b.ino(),
        _ => false,
    };
    let home = dirs::home_dir();
    if home
        .as_ref()
        .is_some_and(|home| real == *home || same_directory(&real, home))
    {
        return true;
    }
    let protected = [
        "/System",
        "/Library",
        "/Applications",
        "/bin",
        "/sbin",
        "/usr",
        "/etc",
        "/private/etc",
        "/private/var/db",
        "/private/var/root",
        "/private/var/vm",
        "/private/var/log",
        "/dev",
        "/cores",
        "/opt",
    ];
    protected
        .iter()
        .map(PathBuf::from)
        .chain(home.map(|home| home.join("Library")))
        .any(|tree| {
            let tree = std::fs::canonicalize(&tree).unwrap_or(tree);
            // Inode-vergelijking dekt ook afwijkende spelling op een hoofdletterongevoelige schijf.
            real.starts_with(&tree)
                || real
                    .ancestors()
                    .any(|ancestor| same_directory(ancestor, &tree))
        })
        || real == Path::new("/Volumes")
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn is_forbidden_root(_path: &Path) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn forbidden_roots() {
        assert!(is_forbidden_root(Path::new("C:\\")));
        assert!(is_forbidden_root(Path::new("C:\\Windows\\System32")));
        assert!(!is_forbidden_root(Path::new("C:\\Users\\x\\Downloads")));
    }

    #[cfg(windows)]
    #[test]
    fn forbidden_drive_roots_in_any_spelling() {
        assert!(is_forbidden_root(Path::new("C:")));
        assert!(is_forbidden_root(Path::new("d:/")));
        assert!(is_forbidden_root(Path::new("D:\\")));
    }

    #[cfg(windows)]
    #[test]
    fn forbidden_windows_and_program_files_trees_case_insensitive() {
        assert!(is_forbidden_root(Path::new("c:\\windows")));
        assert!(is_forbidden_root(Path::new("C:/WINDOWS/System32/")));
        assert!(is_forbidden_root(Path::new("C:\\Program Files")));
        assert!(is_forbidden_root(Path::new("C:\\program files\\App\\data")));
        assert!(is_forbidden_root(Path::new("C:\\Program Files (x86)")));
        assert!(is_forbidden_root(Path::new("C:\\Program Files (x86)\\App")));
    }

    #[cfg(windows)]
    #[test]
    fn lookalike_names_are_not_forbidden() {
        assert!(!is_forbidden_root(Path::new("C:\\Windows.old.backup")));
        assert!(!is_forbidden_root(Path::new("C:\\Program Files Extra")));
        assert!(!is_forbidden_root(Path::new("C:\\Projecten\\Windows")));
    }

    #[test]
    fn forbidden_home_itself_but_not_its_subfolders() {
        let Some(home) = dirs::home_dir() else { return };
        assert!(is_forbidden_root(&home));
        assert!(!is_forbidden_root(&home.join("Downloads")));
        assert!(!is_forbidden_root(&home.join("Desktop").join("Netjes")));
    }

    #[test]
    fn app_data_dir_is_created_and_named_netjes() {
        let dir = app_data_dir();
        assert!(dir.is_dir());
        assert_eq!(dir.file_name().and_then(|n| n.to_str()), Some("Netjes"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn mac_protects_system_trees_home_library_and_mount_roots() {
        for path in [
            "/",
            "/System",
            "/System/Library",
            "/Library",
            "/Applications",
            "/usr/bin",
            "/private/etc",
            "/dev",
            "/Volumes",
            "/System/Volumes/Data",
        ] {
            assert!(is_forbidden_root(Path::new(path)), "{path}");
        }
        let home = dirs::home_dir().unwrap();
        assert!(is_forbidden_root(&home));
        assert!(is_forbidden_root(&home.join("Library")));
        assert!(is_forbidden_root(&home.join("Library/Caches")));
        assert!(!is_forbidden_root(&home.join("Downloads")));
        let tmp = tempfile::tempdir().unwrap();
        assert!(!is_forbidden_root(tmp.path()));
        assert!(!is_forbidden_root(&tmp.path().join("Library")));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn mac_symlinks_and_parent_components_cannot_bypass_protection() {
        use std::os::unix::fs::symlink;
        let tmp = tempfile::tempdir().unwrap();
        let link = tmp.path().join("gewone-map");
        symlink("/System", &link).unwrap();
        assert!(is_forbidden_root(&link));
        assert!(is_forbidden_root(&link.join("Library")));
        assert!(is_forbidden_root(&link.join("Library/../Library")));
        let home_link = tmp.path().join("thuis");
        symlink(dirs::home_dir().unwrap(), &home_link).unwrap();
        assert!(is_forbidden_root(&home_link));
        assert!(is_forbidden_root(&home_link.join("Library")));
    }
}
