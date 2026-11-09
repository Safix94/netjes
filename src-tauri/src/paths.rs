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
    if let (Ok(meta), Some(parent)) = (std::fs::metadata(&real), real.parent())
        && std::fs::metadata(parent).is_ok_and(|parent| meta.dev() != parent.dev())
    {
        return true; // Een mountpoint, ook buiten /Volumes.
    }
    let same_directory = |a: &Path, b: &Path| match (std::fs::metadata(a), std::fs::metadata(b)) {
        (Ok(a), Ok(b)) => a.dev() == b.dev() && a.ino() == b.ino(),
        _ => false,
    };
    // De containers zelf zijn systeemlocaties; gewone mappen eronder (zoals tijdelijke tests) niet.
    if [
        "/Volumes",
        "/Users",
        "/private",
        "/private/var",
        "/private/tmp",
        "/private/var/folders",
    ]
    .iter()
    .any(|path| real == Path::new(path) || same_directory(&real, Path::new(path)))
    {
        return true;
    }
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
            "/private",
            "/var",
            "/Users",
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

    #[cfg(target_os = "macos")]
    #[test]
    fn mac_mounted_case_sensitive_volume_keeps_recovery_logs_separate() {
        use crate::{mover, planner, scan, undo};
        use std::process::Command;

        let temp = tempfile::tempdir().unwrap();
        let image = temp.path().join("test.dmg");
        let mount = temp.path().join("volume");
        std::fs::create_dir(&mount).unwrap();
        let created = Command::new("hdiutil")
            .args([
                "create",
                "-quiet",
                "-size",
                "32m",
                "-fs",
                "HFSX",
                "-volname",
                "NetjesTests",
            ])
            .arg(&image)
            .output()
            .unwrap();
        assert!(
            created.status.success(),
            "{}",
            String::from_utf8_lossy(&created.stderr)
        );
        let attached = Command::new("hdiutil")
            .args(["attach", "-quiet", "-nobrowse", "-mountpoint"])
            .arg(&mount)
            .arg(&image)
            .output()
            .unwrap();
        assert!(
            attached.status.success(),
            "{}",
            String::from_utf8_lossy(&attached.stderr)
        );
        struct MountedVolume(PathBuf);
        impl Drop for MountedVolume {
            fn drop(&mut self) {
                let _ = Command::new("hdiutil")
                    .args(["detach", "-quiet"])
                    .arg(&self.0)
                    .status();
            }
        }
        let _mounted = MountedVolume(mount.clone());
        assert!(is_forbidden_root(&mount));
        let link = temp.path().join("volume-link");
        std::os::unix::fs::symlink(&mount, &link).unwrap();
        assert!(is_forbidden_root(&link));
        let logs = temp.path().join("logs");
        let upper = mount.join("Downloads");
        let lower = mount.join("downloads");
        for (root, content) in [(&upper, b"boven".as_slice()), (&lower, b"onder".as_slice())] {
            std::fs::create_dir(root).unwrap();
            assert!(!is_forbidden_root(root));
            std::fs::write(root.join("a.jpg"), content).unwrap();
            let scanned = scan::scan_directory(root).unwrap();
            let moves = planner::plan_moves(root, &scanned.files);
            let folders =
                mover::ensure_folders(root, &[crate::categories::Category::Fotos]).unwrap();
            undo::write_planned_in(&logs, root, &moves, &folders).unwrap();
            let outcomes = mover::move_many(&moves);
            assert!(outcomes.iter().all(|outcome| outcome.ok), "{outcomes:?}");
            undo::apply_outcomes_in(&logs, root, &outcomes).unwrap();
        }
        assert_ne!(
            undo::log_path_in(&logs, &upper),
            undo::log_path_in(&logs, &lower)
        );
        assert_eq!(undo::restore_in(&logs, &upper, true).unwrap().restored, 1);
        assert_eq!(std::fs::read(upper.join("a.jpg")).unwrap(), b"boven");
        assert!(!lower.join("a.jpg").exists());
        assert_eq!(undo::info_in(&logs, &lower).unwrap().count, 1);
        assert_eq!(undo::restore_in(&logs, &lower, true).unwrap().restored, 1);
        assert_eq!(std::fs::read(lower.join("a.jpg")).unwrap(), b"onder");
    }
}
