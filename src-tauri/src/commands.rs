//! De Tauri-commando's. Elk commando is een dunne `async` laag die het eigenlijke werk als `*_impl`
//! op een blokkerende thread draait; die `*_impl`-functies kennen geen Tauri-runtime en zijn
//! daardoor rechtstreeks te testen.
//!
//! Wat bestanden verplaatst, voert enkel de verplaatsingen uit het plan in de `AppState` uit
//! (opgezocht op `id`), nooit iets wat de UI zelf aanlevert. Slotvolgorde overal: eerst
//! `organize_lock`, dan `last_scan`, dan `plan`; geen enkel slot staat over een `.await`.

use crate::categories::{self, Category, CategoryInfo};
use crate::dupes;
use crate::mover::{self, MoveOutcome};
use crate::paths;
use crate::planner::{self, PlannedMove};
use crate::scan::{self, FileEntry, ScanError, ScanResult};
use crate::state::{AppState, lock};
use crate::thumbs;
use crate::undo::{self, LOG_SAVE_FAILED_ID, UndoInfo, UndoResult};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

const INTERNAL_ERROR: &str = "Er ging iets mis. Probeer het opnieuw.";
const FORBIDDEN_ROOT: &str = "Die map ruimt Netjes niet op. Kies een gewone map zoals Downloads.";
const NOT_A_FOLDER: &str = "Dit is geen map.";
const NOT_SCANNED: &str = "Deze map is nog niet gescand.";
const NO_PLAN: &str = "Eerst opruimen starten.";
const RESCAN_FIRST: &str = "Scan de map opnieuw.";
const UNKNOWN_FILE: &str = "Onbekend bestand.";
const NO_DOWNLOADS: &str = "Downloads-map niet gevonden";
const MAX_THUMB_PX: u32 = 1024;

/// Wat `begin_organize` teruggeeft: alle geplande verplaatsingen en de mappen die Netjes zelf
/// heeft aangemaakt.
#[derive(Serialize, Debug)]
pub struct OrganizePlan {
    pub moves: Vec<PlannedMove>,
    pub folders_created: Vec<String>,
}

// --- Tauri-commando's -------------------------------------------------------------------------

#[tauri::command]
pub async fn get_downloads_dir() -> Result<String, String> {
    let downloads = paths::downloads_dir().ok_or(NO_DOWNLOADS)?;
    Ok(root_string(&normalize_root(&downloads.to_string_lossy())?))
}

#[tauri::command]
pub async fn get_categories() -> Vec<CategoryInfo> {
    categories::infos()
}

#[tauri::command]
pub async fn scan(app: AppHandle, root: String) -> Result<ScanResult, String> {
    with_state(app, move |state| scan_impl(state, &root)).await?
}

#[tauri::command]
pub async fn begin_organize(app: AppHandle, root: String) -> Result<OrganizePlan, String> {
    with_state(app, move |state| begin_organize_impl(state, &root)).await?
}

#[tauri::command]
pub async fn move_files(
    app: AppHandle,
    root: String,
    ids: Vec<u32>,
) -> Result<Vec<MoveOutcome>, String> {
    with_state(app, move |state| move_files_impl(state, &root, &ids)).await?
}

#[tauri::command]
pub async fn check_undo(app: AppHandle, root: String) -> Option<UndoInfo> {
    with_state(app, move |state| check_undo_impl(state, &root))
        .await
        .ok()
        .flatten()
}

#[tauri::command]
pub async fn undo_organize(app: AppHandle, root: String) -> Result<UndoResult, String> {
    with_state(app, move |state| undo_organize_impl(state, &root)).await?
}

#[tauri::command]
pub async fn disorganize(app: AppHandle, root: String) -> Result<UndoResult, String> {
    with_state(app, move |state| disorganize_impl(state, &root)).await?
}

#[tauri::command]
pub async fn list_folder_files(root: String, category: Category) -> Result<Vec<FileEntry>, String> {
    blocking(move || list_folder_files_impl(&root, category)).await?
}

/// De paden komen ongewijzigd binnen en gaan ongewijzigd naar de shell; ze worden nooit
/// gecanoniseerd. Bij een mislukte thread blijft de lengte van het antwoord kloppen.
#[tauri::command]
pub async fn get_thumbnails(paths: Vec<String>, px: u32) -> Vec<Option<String>> {
    let px = px.clamp(1, MAX_THUMB_PX);
    let count = paths.len();
    blocking(move || thumbs::get_thumbnails(&paths, px))
        .await
        .unwrap_or_else(|_| vec![None; count])
}

// --- Draaien op een blokkerende thread --------------------------------------------------------

async fn blocking<T: Send + 'static>(
    job: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(job)
        .await
        .map_err(|_| INTERNAL_ERROR.to_string())
}

async fn with_state<T: Send + 'static>(
    app: AppHandle,
    job: impl FnOnce(&AppState) -> T + Send + 'static,
) -> Result<T, String> {
    blocking(move || {
        let state = app.state::<AppState>();
        job(&state)
    })
    .await
}

// --- De eigenlijke logica ---------------------------------------------------------------------

/// De ene vorm van een mappad waarmee alles werkt: absoluut, zonder `\\?\`-voorvoegsel en zonder
/// slotscheidingsteken. De undo-logs zijn op deze tekst gehasht, dus hij moet overal gelijk zijn.
fn normalize_root(root: &str) -> Result<PathBuf, String> {
    if root.is_empty() {
        return Err(NOT_A_FOLDER.to_string());
    }
    #[cfg(not(windows))]
    {
        // realpath behoudt Unix-padbytes, hoofdletters en de wortel `/`.
        std::fs::canonicalize(root).map_err(|_| NOT_A_FOLDER.to_string())
    }
    #[cfg(windows)]
    {
        let absolute = std::path::absolute(root).map_err(|_| NOT_A_FOLDER.to_string())?;
        let text = absolute.to_string_lossy();
        let text = if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
            format!(r"\\{rest}")
        } else if let Some(rest) = text.strip_prefix(r"\\?\") {
            rest.to_string()
        } else {
            text.into_owned()
        };
        let trimmed = text.trim_end_matches(['\\', '/']);
        // `C:` zonder scheidingsteken is de huidige map van die schijf; een schijfwortel houdt zijn teken.
        Ok(if trimmed.ends_with(':') {
            PathBuf::from(format!(r"{trimmed}\"))
        } else {
            PathBuf::from(trimmed)
        })
    }
}

fn root_string(root: &Path) -> String {
    root.to_string_lossy().into_owned()
}

fn scan_error_text(error: ScanError) -> String {
    match error {
        ScanError::Forbidden => FORBIDDEN_ROOT.to_string(),
        ScanError::NotADirectory => NOT_A_FOLDER.to_string(),
        io @ ScanError::Io(_) => io.to_string(),
    }
}

fn scan_impl(state: &AppState, root: &str) -> Result<ScanResult, String> {
    let root = normalize_root(root)?;
    let mut result = scan::scan_directory(&root).map_err(scan_error_text)?;
    dupes::mark_duplicates(&mut result.files);

    let mut last_scan = lock(&state.last_scan);
    let mut plan = lock(&state.plan);
    *last_scan = Some(result.clone());
    // Een plan hoort bij de scan waaruit het kwam.
    *plan = None;
    Ok(result)
}

fn begin_organize_impl(state: &AppState, root: &str) -> Result<OrganizePlan, String> {
    let root = normalize_root(root)?;
    let _organizing = lock(&state.organize_lock);
    // Dit slot blijft vast tot het plan klaar is, zodat er geen nieuwe scan tussen kan komen.
    let last_scan = lock(&state.last_scan);
    let scanned = last_scan
        .as_ref()
        .filter(|scan| scan.root == root_string(&root))
        .ok_or(NOT_SCANNED)?;
    // Een bestaand plan hoort bij een log dat al verplaatste bestanden kan bevatten. Een tweede
    // start op dezelfde scan zou dat log vervangen en de undo van die bestanden kwijtraken; elke
    // eerlijke herstart (nieuwe scan, ongedaan maken, rommelig maken) wist het plan eerst.
    if lock(&state.plan).is_some() {
        return Err(RESCAN_FIRST.to_string());
    }

    let moves = planner::plan_moves(&root, &scanned.files);
    if moves.is_empty() {
        // Niets te doen mag een bestaand log nooit vervangen.
        return Ok(OrganizePlan {
            moves,
            folders_created: Vec::new(),
        });
    }
    let used: Vec<Category> = categories::all()
        .iter()
        .copied()
        .filter(|&category| moves.iter().any(|m| m.category == category))
        .collect();
    let created = mover::ensure_folders(&root, &used)
        .map_err(|e| format!("De categoriemappen konden niet aangemaakt worden: {e}"))?;
    undo::write_planned_in(&state.log_dir(), &root, &moves, &created)
        .map_err(|e| format!("Het ongedaan-maken-log kon niet bewaard worden: {e}"))?;

    *lock(&state.plan) = Some(moves.clone());
    Ok(OrganizePlan {
        moves,
        folders_created: created.iter().map(|folder| root_string(folder)).collect(),
    })
}

fn move_files_impl(state: &AppState, root: &str, ids: &[u32]) -> Result<Vec<MoveOutcome>, String> {
    let root = normalize_root(root)?;
    let _organizing = lock(&state.organize_lock);
    let last_scan = lock(&state.last_scan);
    if last_scan
        .as_ref()
        .is_none_or(|scan| scan.root != root_string(&root))
    {
        return Err(NOT_SCANNED.to_string());
    }

    // Eerst alles controleren, dan pas iets verplaatsen: één onbekend id stopt de hele aanroep.
    let selected = {
        let plan = lock(&state.plan);
        let planned: HashMap<u32, &PlannedMove> = plan
            .as_ref()
            .ok_or(NO_PLAN)?
            .iter()
            .map(|m| (m.id, m))
            .collect();
        let mut seen = HashSet::new();
        let mut selected = Vec::new();
        for &id in ids {
            // Hetzelfde id twee keer verplaatsen zou het log van een gelukte move overschrijven.
            if seen.insert(id) {
                selected.push((*planned.get(&id).ok_or(UNKNOWN_FILE)?).clone());
            }
        }
        selected
    };
    if selected.is_empty() {
        return Ok(Vec::new());
    }

    let mut outcomes = mover::move_many(&selected);
    if let Err(e) = undo::apply_outcomes_in(&state.log_dir(), &root, &outcomes) {
        // De bestanden zijn al verplaatst; meld dat het log dat niet weet, op dezelfde manier als
        // `undo::restore` dat doet.
        outcomes.push(MoveOutcome {
            id: LOG_SAVE_FAILED_ID,
            ok: false,
            dest: None,
            error: Some(format!("Het log kon niet bewaard worden: {e}")),
        });
    }

    // Wat verplaatst is, staat niet meer in het plan: nog een keer verplaatsen kan dus niet.
    let moved: HashSet<u32> = outcomes.iter().filter(|o| o.ok).map(|o| o.id).collect();
    if let Some(plan) = lock(&state.plan).as_mut() {
        plan.retain(|m| !moved.contains(&m.id));
    }
    Ok(outcomes)
}

fn check_undo_impl(state: &AppState, root: &str) -> Option<UndoInfo> {
    let root = normalize_root(root).ok()?;
    undo::info_in(&state.log_dir(), &root)
}

/// Zet alles terug en ruimt de door Netjes aangemaakte lege mappen op.
fn undo_organize_impl(state: &AppState, root: &str) -> Result<UndoResult, String> {
    restore_impl(state, root, true)
}

/// Zet alles terug maar laat alle mappen staan (rommelig maken voor de opnamemodus).
fn disorganize_impl(state: &AppState, root: &str) -> Result<UndoResult, String> {
    restore_impl(state, root, false)
}

fn restore_impl(
    state: &AppState,
    root: &str,
    remove_created_folders: bool,
) -> Result<UndoResult, String> {
    let root = normalize_root(root)?;
    let _organizing = lock(&state.organize_lock);
    let restored = undo::restore_in(&state.log_dir(), &root, remove_created_folders);
    // Na het terugzetten klopt het plan niet meer; een nieuwe ronde begint met `begin_organize`.
    *lock(&state.plan) = None;
    restored.map_err(|e| format!("Het terugzetten is mislukt: {e}"))
}

fn list_folder_files_impl(root: &str, category: Category) -> Result<Vec<FileEntry>, String> {
    let root = normalize_root(root)?;
    if paths::is_forbidden_root(&root) {
        return Err(FORBIDDEN_ROOT.to_string());
    }
    scan::scan_directory(&root.join(categories::folder_name(category)))
        .map(|listing| listing.files)
        .map_err(scan_error_text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{Duration, SystemTime};
    use tempfile::{TempDir, tempdir};

    #[cfg(target_os = "macos")]
    #[test]
    fn mac_canonicalization_permission_denial_has_recovery_instead_of_not_a_folder() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempdir().unwrap();
        let parent = tmp.path().join("afgesloten");
        let child = parent.join("Downloads");
        fs::create_dir_all(&child).unwrap();
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o000)).unwrap();
        let normalized = normalize_root(&child.to_string_lossy());
        let scanned = scan_impl(&AppState::default(), &child.to_string_lossy());
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(normalized.unwrap_err().contains("Systeeminstellingen"));
        assert!(scanned.unwrap_err().contains("Systeeminstellingen"));
    }

    struct Fixture {
        root_dir: TempDir,
        _logs: TempDir,
        state: AppState,
    }

    impl Fixture {
        /// `a.jpg`, `b.pdf` en `a (1).jpg` (zelfde inhoud als `a.jpg`, maar nieuwer).
        fn new() -> Self {
            let root_dir = tempdir().unwrap();
            let logs = tempdir().unwrap();
            let root = root_dir.path();
            fs::write(root.join("a.jpg"), b"zelfde inhoud").unwrap();
            fs::write(root.join("a (1).jpg"), b"zelfde inhoud").unwrap();
            fs::write(root.join("b.pdf"), b"pdf-inhoud, anders").unwrap();
            let old = SystemTime::now() - Duration::from_secs(3600);
            fs::File::options()
                .write(true)
                .open(root.join("a.jpg"))
                .unwrap()
                .set_modified(old)
                .unwrap();
            let state = AppState {
                log_dir: Some(logs.path().to_path_buf()),
                ..AppState::default()
            };
            Self {
                root_dir,
                _logs: logs,
                state,
            }
        }

        fn root(&self) -> &Path {
            self.root_dir.path()
        }

        fn root_str(&self) -> String {
            root_string(&normalize_root(&root_string(self.root())).unwrap())
        }

        /// Scant en start het opruimen; geeft de gescande root en het plan terug.
        fn scanned_and_planned(&self) -> (String, OrganizePlan) {
            let scan = scan_impl(&self.state, &self.root_str()).unwrap();
            let plan = begin_organize_impl(&self.state, &scan.root).unwrap();
            (scan.root, plan)
        }

        fn id_of(plan: &OrganizePlan, name: &str) -> u32 {
            plan.moves
                .iter()
                .find(|m| m.src.ends_with(name))
                .unwrap_or_else(|| panic!("{name} staat niet in het plan"))
                .id
        }
    }

    #[test]
    fn scan_organize_undo_chain() {
        let fx = Fixture::new();
        let root = fx.root().to_path_buf();

        let scan = scan_impl(&fx.state, &fx.root_str()).unwrap();
        assert_eq!(scan.files.len(), 3);
        let duplicate = scan.files.iter().find(|f| f.name == "a (1).jpg").unwrap();
        assert!(duplicate.is_duplicate);
        assert_eq!(duplicate.category, Category::Duplicaten);

        let plan = begin_organize_impl(&fx.state, &scan.root).unwrap();
        assert_eq!(plan.moves.len(), 3);
        assert_eq!(plan.folders_created.len(), 3);
        let ids: Vec<u32> = plan.moves.iter().map(|m| m.id).collect();

        let outcomes = move_files_impl(&fx.state, &scan.root, &ids).unwrap();
        assert_eq!(outcomes.len(), 3);
        assert!(outcomes.iter().all(|o| o.ok), "{outcomes:?}");
        assert!(root.join("Foto's").join("a.jpg").is_file());
        assert!(root.join("PDF's").join("b.pdf").is_file());
        assert!(root.join("Duplicaten").join("a (1).jpg").is_file());
        assert!(!root.join("a.jpg").exists());

        let info = check_undo_impl(&fx.state, &scan.root).unwrap();
        assert_eq!(info.count, 3);

        let undone = undo_organize_impl(&fx.state, &scan.root).unwrap();
        assert_eq!(undone.restored, 3);
        assert!(undone.errors.is_empty());
        assert!(root.join("a.jpg").is_file());
        assert!(root.join("a (1).jpg").is_file());
        assert!(root.join("b.pdf").is_file());
        assert!(!root.join("Foto's").exists());
        assert_eq!(check_undo_impl(&fx.state, &scan.root).unwrap().count, 0);
    }

    #[test]
    fn scan_stores_and_returns_one_canonical_root() {
        let fx = Fixture::new();
        let with_slash = format!("{}{sep}", fx.root_str(), sep = std::path::MAIN_SEPARATOR);
        let scan = scan_impl(&fx.state, &with_slash).unwrap();
        assert_eq!(scan.root, fx.root_str());
        assert!(!scan.root.starts_with(r"\\?\"));
        let stored = lock(&fx.state.last_scan);
        assert_eq!(stored.as_ref().unwrap().root, scan.root);
    }

    #[cfg(windows)]
    #[test]
    fn normalize_root_strips_verbatim_prefix_and_trailing_separators() {
        assert_eq!(
            normalize_root(r"\\?\C:\Gebruikers\Downloads\\").unwrap(),
            PathBuf::from(r"C:\Gebruikers\Downloads")
        );
        assert_eq!(
            normalize_root(r"\\?\UNC\server\share\map").unwrap(),
            PathBuf::from(r"\\server\share\map")
        );
        assert_eq!(normalize_root("C:/").unwrap(), PathBuf::from(r"C:\"));
        assert_eq!(normalize_root("").unwrap_err(), NOT_A_FOLDER);
    }

    #[test]
    fn scan_refuses_forbidden_roots_and_non_folders() {
        let fx = Fixture::new();
        #[cfg(windows)]
        let protected = [r"C:\Windows", r"C:\"];
        #[cfg(not(windows))]
        let protected = ["/System", "/"];
        for path in protected {
            assert_eq!(scan_impl(&fx.state, path).unwrap_err(), FORBIDDEN_ROOT);
        }
        let file = fx.root().join("b.pdf");
        assert_eq!(
            scan_impl(&fx.state, &root_string(&file)).unwrap_err(),
            NOT_A_FOLDER
        );
        assert!(lock(&fx.state.last_scan).is_none());
    }

    #[test]
    fn begin_organize_needs_a_scan_of_the_same_root() {
        let fx = Fixture::new();
        let other = tempdir().unwrap();
        assert_eq!(
            begin_organize_impl(&fx.state, &fx.root_str()).unwrap_err(),
            NOT_SCANNED
        );
        scan_impl(&fx.state, &fx.root_str()).unwrap();
        assert_eq!(
            begin_organize_impl(&fx.state, &root_string(other.path())).unwrap_err(),
            NOT_SCANNED
        );
        // Er is niets aangemaakt en geen plan bewaard.
        assert!(!fx.root().join("Foto's").exists());
        assert!(lock(&fx.state.plan).is_none());
    }

    #[test]
    fn move_files_needs_a_plan_for_the_scanned_root() {
        let fx = Fixture::new();
        let scan = scan_impl(&fx.state, &fx.root_str()).unwrap();
        assert_eq!(
            move_files_impl(&fx.state, &scan.root, &[0]).unwrap_err(),
            NO_PLAN
        );

        let (_, plan) = fx.scanned_and_planned();
        let other = tempdir().unwrap();
        let id = Fixture::id_of(&plan, "b.pdf");
        assert_eq!(
            move_files_impl(&fx.state, &root_string(other.path()), &[id]).unwrap_err(),
            NOT_SCANNED
        );
        assert!(fx.root().join("b.pdf").is_file());
    }

    #[test]
    fn move_files_refuses_unknown_ids_before_moving_anything() {
        let fx = Fixture::new();
        let (root, plan) = fx.scanned_and_planned();
        let id = Fixture::id_of(&plan, "b.pdf");
        assert_eq!(
            move_files_impl(&fx.state, &root, &[id, 99]).unwrap_err(),
            UNKNOWN_FILE
        );
        assert!(fx.root().join("b.pdf").is_file());
        assert_eq!(check_undo_impl(&fx.state, &root).unwrap().count, 0);
    }

    #[test]
    fn move_files_moves_only_the_chosen_files_and_only_once() {
        let fx = Fixture::new();
        let (root, plan) = fx.scanned_and_planned();
        let pdf = Fixture::id_of(&plan, "b.pdf");

        let outcomes = move_files_impl(&fx.state, &root, &[pdf, pdf]).unwrap();
        assert_eq!(outcomes.len(), 1);
        assert!(outcomes[0].ok);
        assert!(fx.root().join("PDF's").join("b.pdf").is_file());
        assert!(fx.root().join("a.jpg").is_file());
        assert_eq!(check_undo_impl(&fx.state, &root).unwrap().count, 1);

        // Hetzelfde id nog eens kan niet; het log van de gelukte move blijft dus kloppen.
        assert_eq!(
            move_files_impl(&fx.state, &root, &[pdf]).unwrap_err(),
            UNKNOWN_FILE
        );
        assert_eq!(check_undo_impl(&fx.state, &root).unwrap().count, 1);
        assert!(move_files_impl(&fx.state, &root, &[]).unwrap().is_empty());
    }

    #[test]
    fn a_new_scan_drops_the_plan() {
        let fx = Fixture::new();
        let (root, plan) = fx.scanned_and_planned();
        let id = Fixture::id_of(&plan, "b.pdf");
        scan_impl(&fx.state, &root).unwrap();
        assert_eq!(
            move_files_impl(&fx.state, &root, &[id]).unwrap_err(),
            NO_PLAN
        );
    }

    #[test]
    fn begin_organize_twice_on_the_same_scan_keeps_the_undo_log() {
        let fx = Fixture::new();
        let (root, plan) = fx.scanned_and_planned();
        let ids: Vec<u32> = plan.moves.iter().map(|m| m.id).collect();
        move_files_impl(&fx.state, &root, &ids).unwrap();
        assert_eq!(check_undo_impl(&fx.state, &root).unwrap().count, 3);

        // Zonder nieuwe scan klopt het plan niet meer en mag het log niet vervangen worden.
        assert_eq!(
            begin_organize_impl(&fx.state, &root).unwrap_err(),
            RESCAN_FIRST
        );
        assert_eq!(check_undo_impl(&fx.state, &root).unwrap().count, 3);

        // Na een nieuwe scan is er niets meer los: een leeg plan, en het log blijft gewoon staan.
        scan_impl(&fx.state, &root).unwrap();
        let empty = begin_organize_impl(&fx.state, &root).unwrap();
        assert!(empty.moves.is_empty());
        assert!(empty.folders_created.is_empty());
        assert_eq!(check_undo_impl(&fx.state, &root).unwrap().count, 3);
        assert_eq!(undo_organize_impl(&fx.state, &root).unwrap().restored, 3);
    }

    #[test]
    fn begin_organize_refuses_a_second_start_before_anything_moved() {
        let fx = Fixture::new();
        let (root, plan) = fx.scanned_and_planned();
        assert_eq!(
            begin_organize_impl(&fx.state, &root).unwrap_err(),
            RESCAN_FIRST
        );
        // Het eerste plan is nog bruikbaar.
        let id = Fixture::id_of(&plan, "b.pdf");
        assert!(move_files_impl(&fx.state, &root, &[id]).unwrap()[0].ok);
        assert_eq!(check_undo_impl(&fx.state, &root).unwrap().count, 1);
    }

    #[test]
    fn undo_log_is_found_through_any_spelling_of_the_root() {
        let fx = Fixture::new();
        let (root, plan) = fx.scanned_and_planned();
        let ids: Vec<u32> = plan.moves.iter().map(|m| m.id).collect();
        move_files_impl(&fx.state, &root, &ids).unwrap();

        let trailing_backslash = format!("{root}{sep}", sep = std::path::MAIN_SEPARATOR);
        let forward_slashes = format!("{}/", root.replace('\\', "/"));
        for spelling in [&trailing_backslash, &forward_slashes] {
            assert_eq!(
                check_undo_impl(&fx.state, spelling).unwrap().count,
                3,
                "{spelling}"
            );
        }

        let undone = undo_organize_impl(&fx.state, &forward_slashes).unwrap();
        assert_eq!(undone.restored, 3);
        assert!(undone.errors.is_empty());
        assert!(fx.root().join("a.jpg").is_file());
        assert!(fx.root().join("a (1).jpg").is_file());
        assert!(fx.root().join("b.pdf").is_file());
        assert_eq!(
            check_undo_impl(&fx.state, &trailing_backslash)
                .unwrap()
                .count,
            0
        );
    }

    #[test]
    fn disorganize_puts_files_back_but_keeps_the_folders() {
        let fx = Fixture::new();
        let (root, plan) = fx.scanned_and_planned();
        let ids: Vec<u32> = plan.moves.iter().map(|m| m.id).collect();
        move_files_impl(&fx.state, &root, &ids).unwrap();

        let result = disorganize_impl(&fx.state, &root).unwrap();
        assert_eq!(result.restored, 3);
        assert!(fx.root().join("b.pdf").is_file());
        assert!(fx.root().join("PDF's").is_dir());
        assert_eq!(check_undo_impl(&fx.state, &root).unwrap().count, 0);
        assert!(lock(&fx.state.plan).is_none());
    }

    #[test]
    fn list_folder_files_lists_one_category_folder() {
        let fx = Fixture::new();
        let (root, plan) = fx.scanned_and_planned();
        let ids: Vec<u32> = plan.moves.iter().map(|m| m.id).collect();
        move_files_impl(&fx.state, &root, &ids).unwrap();

        let photos = list_folder_files_impl(&root, Category::Fotos).unwrap();
        assert_eq!(photos.len(), 1);
        assert_eq!(photos[0].name, "a.jpg");
        assert_eq!(photos[0].id, 0);
        assert_eq!(
            list_folder_files_impl(&root, Category::Audio).unwrap_err(),
            NOT_A_FOLDER
        );
        #[cfg(windows)]
        assert_eq!(
            list_folder_files_impl(r"C:\Windows", Category::Fotos).unwrap_err(),
            FORBIDDEN_ROOT
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn mac_normalize_keeps_root_case_and_resolves_existing_symlinks() {
        use std::os::unix::fs::symlink;
        assert_eq!(normalize_root("/").unwrap(), Path::new("/"));
        assert!(normalize_root("").is_err());
        let tmp = tempdir().unwrap();
        let actual = tmp.path().join("DownloadsUpper");
        fs::create_dir(&actual).unwrap();
        let alias = tmp.path().join("link");
        symlink(&actual, &alias).unwrap();
        assert_eq!(
            normalize_root(&format!("{}/", alias.display())).unwrap(),
            fs::canonicalize(&actual).unwrap()
        );
        assert!(
            normalize_root(&actual.to_string_lossy())
                .unwrap()
                .ends_with("DownloadsUpper")
        );
    }
}

/// Versiecheck tegen de nieuwste GitHub-release; enkel een melding, geen installatie.
#[tauri::command]
pub async fn check_update() -> crate::update::UpdateInfo {
    let current = env!("CARGO_PKG_VERSION");
    tauri::async_runtime::spawn_blocking(move || crate::update::check(current))
        .await
        .unwrap_or(crate::update::UpdateInfo {
            state: crate::update::UpdateState::Error,
            version: None,
        })
}
