use std::path::{Path, PathBuf};

/// Standaardlocaties van de systeemmappen, voor als de omgevingsvariabelen ontbreken.
const DEFAULT_PROTECTED_TREES: &[&str] = &[
    "c:\\windows",
    "c:\\program files",
    "c:\\program files (x86)",
];

/// Omgevingsvariabelen die een beschermde mapboom aanwijzen.
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
pub fn is_forbidden_root(p: &Path) -> bool {
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
fn normalize(path: &str) -> String {
    let lower = path.to_lowercase().replace('/', "\\");
    let without_prefix = lower.strip_prefix("\\\\?\\").unwrap_or(&lower);
    without_prefix.trim_end_matches('\\').to_string()
}

/// `c:` (het genormaliseerde `C:\`): een letter, een dubbele punt en verder niets.
fn is_drive_root(normalized: &str) -> bool {
    let mut chars = normalized.chars();
    matches!(
        (chars.next(), chars.next(), chars.next()),
        (Some(letter), Some(':'), None) if letter.is_ascii_alphabetic()
    )
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
        for path in ["/", "/System", "/System/Library", "/Library", "/Applications", "/usr/bin", "/private/etc", "/dev", "/Volumes", "/System/Volumes/Data"] {
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
