//! Het ongedaan-maken-log: per gekozen map één bestand in `%LOCALAPPDATA%\Netjes\undo`. Het is de
//! bron van waarheid voor "Ongedaan maken" en, in opnamemodus, voor "Rommelig maken". Terugzetten
//! gebruikt dezelfde no-replace-verplaatsing als het opruimen zelf: er wordt nooit iets overschreven
//! en nooit iets verwijderd, behalve lege mappen die Netjes zelf aanmaakte.

use crate::mover::{MoveOutcome, OUTSIDE, move_path_no_replace};
use crate::paths::app_data_dir;
use crate::planner::PlannedMove;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const LOG_VERSION: u32 = 1;
/// Na zoveel teruggezette bestanden gaat het log al naar schijf, zodat een crash of een mislukte
/// laatste `save` nooit meer dan dit aantal bestanden als `Moved` laat staan terwijl ze al terug zijn.
const CHECKPOINT_EVERY: usize = 50;
/// `MoveOutcome::id` van de fout "het log kon niet bewaard worden"; geen echt record heeft dit id.
pub const LOG_SAVE_FAILED_ID: u32 = u32::MAX;
const NO_LOG: &str = "Er is geen ongedaan-maken-log voor deze map";

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RecordStatus {
    Planned,
    Moved,
    Failed,
    Restored,
}

/// Eén bestand: `src` is waar het stond, `dest` waar het nu staat (na conflictoplossing).
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct UndoRecord {
    pub id: u32,
    pub src: String,
    pub dest: String,
    pub status: RecordStatus,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct UndoLog {
    pub version: u32,
    pub root: String,
    pub created_at_ms: u64,
    /// Enkel de mappen die Netjes zelf aanmaakte; alleen die mogen bij ongedaan maken weg.
    pub folders_created: Vec<String>,
    pub records: Vec<UndoRecord>,
}

#[derive(Serialize, Debug)]
pub struct UndoInfo {
    /// Het aantal bestanden dat nog teruggezet kan worden (status `Moved`).
    pub count: u32,
    pub created_at_ms: u64,
}

#[derive(Serialize, Debug)]
pub struct UndoResult {
    pub restored: u32,
    /// Per mislukt bestand een uitkomst met zijn `id`. Kon het log zelf niet bewaard worden, dan
    /// staat er één extra uitkomst met `id == LOG_SAVE_FAILED_ID`; `restored` blijft dan kloppen.
    pub errors: Vec<MoveOutcome>,
}

fn default_log_dir() -> PathBuf {
    app_data_dir().join("undo")
}

/// `app_data_dir()/undo/<blake3 hex van het pad in kleine letters>.json`.
pub fn log_path(root: &Path) -> PathBuf {
    log_path_in(&default_log_dir(), root)
}

/// Zoals `log_path`, maar met een eigen logmap (voor tests).
pub fn log_path_in(log_dir: &Path, root: &Path) -> PathBuf {
    let key = root.to_string_lossy().to_lowercase();
    log_dir.join(format!("{}.json", blake3::hash(key.as_bytes()).to_hex()))
}

/// Begint een nieuw log voor `root` en vervangt het vorige. Alle records staan op `Planned`.
pub fn write_planned(
    root: &Path,
    moves: &[PlannedMove],
    folders_created: &[PathBuf],
) -> io::Result<UndoLog> {
    write_planned_in(&default_log_dir(), root, moves, folders_created)
}

pub fn write_planned_in(
    log_dir: &Path,
    root: &Path,
    moves: &[PlannedMove],
    folders_created: &[PathBuf],
) -> io::Result<UndoLog> {
    let log = UndoLog {
        version: LOG_VERSION,
        root: root.to_string_lossy().into_owned(),
        created_at_ms: now_ms(),
        folders_created: folders_created
            .iter()
            .map(|f| f.to_string_lossy().into_owned())
            .collect(),
        records: moves
            .iter()
            .map(|m| UndoRecord {
                id: m.id,
                src: m.src.clone(),
                dest: m.dest.clone(),
                status: RecordStatus::Planned,
            })
            .collect(),
    };
    save(log_dir, root, &log)?;
    Ok(log)
}

/// Legt de uitkomsten vast per `id`: gelukt wordt `Moved` met de echte bestemming, al de rest `Failed`.
pub fn apply_outcomes(root: &Path, outcomes: &[MoveOutcome]) -> io::Result<()> {
    apply_outcomes_in(&default_log_dir(), root, outcomes)
}

pub fn apply_outcomes_in(log_dir: &Path, root: &Path, outcomes: &[MoveOutcome]) -> io::Result<()> {
    let mut log =
        load_in(log_dir, root)?.ok_or_else(|| io::Error::new(ErrorKind::NotFound, NO_LOG))?;
    let by_id: HashMap<u32, &MoveOutcome> = outcomes.iter().map(|o| (o.id, o)).collect();
    for record in &mut log.records {
        let Some(outcome) = by_id.get(&record.id) else {
            continue;
        };
        // Zonder echte bestemming weten we niet welk bestand we zouden terugzetten: dan liever
        // niets doen dan een ander bestand met dezelfde naam verplaatsen.
        match (outcome.ok, &outcome.dest) {
            (true, Some(dest)) => {
                record.dest = dest.clone();
                record.status = RecordStatus::Moved;
            }
            _ => record.status = RecordStatus::Failed,
        }
    }
    save(log_dir, root, &log)
}

/// `Ok(None)` als er voor deze map nog geen log is.
pub fn load(root: &Path) -> io::Result<Option<UndoLog>> {
    load_in(&default_log_dir(), root)
}

pub fn load_in(log_dir: &Path, root: &Path) -> io::Result<Option<UndoLog>> {
    match fs::read(log_path_in(log_dir, root)) {
        Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// `None` enkel als er geen (leesbaar) log is; na een volledige restore is `count` 0.
pub fn info(root: &Path) -> Option<UndoInfo> {
    info_in(&default_log_dir(), root)
}

pub fn info_in(log_dir: &Path, root: &Path) -> Option<UndoInfo> {
    let log = load_in(log_dir, root).ok().flatten()?;
    let moved = log
        .records
        .iter()
        .filter(|r| r.status == RecordStatus::Moved)
        .count();
    Some(UndoInfo {
        count: u32::try_from(moved).unwrap_or(u32::MAX),
        created_at_ms: log.created_at_ms,
    })
}

/// Zet elk `Moved`-bestand terug naar zijn oorspronkelijke plek, of naar `naam (2).ext` als daar
/// intussen weer iets staat. Gelukt wordt `Restored`; mislukt blijft `Moved` en komt in `errors`.
/// Met `remove_created_folders` (ongedaan maken) verdwijnen daarna de door Netjes aangemaakte
/// mappen die nu leeg zijn; zonder (rommelig maken) blijven alle mappen staan. Het log blijft bestaan.
///
/// Enkel bestanden waarvan `src` en `dest` in `root` liggen worden aangeraakt; de rest krijgt een
/// fout "buiten de map". Het log gaat om de `CHECKPOINT_EVERY` bestanden en op het einde naar schijf.
/// Mislukt dat bewaren, dan stopt het terugzetten en komt de fout als uitkomst met
/// `LOG_SAVE_FAILED_ID` in `errors`: een `Ok` met correcte `restored`, nooit een kale `Err`
/// terwijl er al bestanden verplaatst zijn.
pub fn restore(root: &Path, remove_created_folders: bool) -> io::Result<UndoResult> {
    restore_in(&default_log_dir(), root, remove_created_folders)
}

pub fn restore_in(
    log_dir: &Path,
    root: &Path,
    remove_created_folders: bool,
) -> io::Result<UndoResult> {
    let Some(mut log) = load_in(log_dir, root)? else {
        return Ok(UndoResult {
            restored: 0,
            errors: Vec::new(),
        });
    };
    let canonical_root = fs::canonicalize(root)?;
    let moved: Vec<usize> = log
        .records
        .iter()
        .enumerate()
        .filter(|(_, r)| r.status == RecordStatus::Moved)
        .map(|(index, _)| index)
        .collect();
    let mut restored = 0;
    let mut errors = Vec::new();
    let mut unsaved = 0;
    let mut checkpoint_failed = false;
    for index in moved {
        let Some(record) = log.records.get_mut(index) else {
            continue;
        };
        match restore_record(record, &canonical_root) {
            Ok(()) => {
                record.status = RecordStatus::Restored;
                restored += 1;
                unsaved += 1;
            }
            Err(error) => errors.push(MoveOutcome {
                id: record.id,
                ok: false,
                dest: None,
                error: Some(error),
            }),
        }
        if unsaved >= CHECKPOINT_EVERY {
            match save(log_dir, root, &log) {
                Ok(()) => unsaved = 0,
                Err(e) => {
                    errors.push(save_failure(&e));
                    checkpoint_failed = true;
                    break;
                }
            }
        }
    }
    if remove_created_folders {
        for folder in &log.folders_created {
            remove_created_folder(folder, &canonical_root);
        }
    }
    if unsaved > 0 {
        // Na een mislukt checkpoint is de fout al gemeld; deze laatste poging redt wat nog kan.
        match save(log_dir, root, &log) {
            Err(e) if !checkpoint_failed => errors.push(save_failure(&e)),
            _ => {}
        }
    }
    Ok(UndoResult { restored, errors })
}

/// Zet één record terug, maar enkel als de mappen van `src` en `dest` in `root` liggen.
/// `root` is al gecanoniseerd.
fn restore_record(record: &UndoRecord, root: &Path) -> Result<(), String> {
    let (src, dest) = (Path::new(&record.src), Path::new(&record.dest));
    ensure_parent_inside(src, root)?;
    ensure_parent_inside(dest, root)?;
    move_path_no_replace(dest, src)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn ensure_parent_inside(path: &Path, root: &Path) -> Result<(), String> {
    let parent = path.parent().ok_or(OUTSIDE)?;
    let parent = fs::canonicalize(parent).map_err(|e| e.to_string())?;
    if parent.starts_with(root) {
        Ok(())
    } else {
        Err(OUTSIDE.to_string())
    }
}

/// Verwijdert een door Netjes aangemaakte map, maar enkel als ze onder `root` ligt (nooit `root`
/// zelf) en leeg is: `remove_dir` weigert een map met inhoud, precies de bescherming die we willen.
fn remove_created_folder(folder: &str, root: &Path) {
    let Ok(folder) = fs::canonicalize(folder) else {
        return;
    };
    if folder != root && folder.starts_with(root) {
        let _ = fs::remove_dir(&folder);
    }
}

fn save_failure(error: &io::Error) -> MoveOutcome {
    MoveOutcome {
        id: LOG_SAVE_FAILED_ID,
        ok: false,
        dest: None,
        error: Some(format!("Het log kon niet bewaard worden: {error}")),
    }
}

/// Schrijft naar `<naam>.json.tmp` en hernoemt dat over het echte log, zodat een crash nooit een
/// half log achterlaat. Hier mag `fs::rename` wél vervangen: het is ons eigen vorige log.
fn save(log_dir: &Path, root: &Path, log: &UndoLog) -> io::Result<()> {
    fs::create_dir_all(log_dir)?;
    let path = log_path_in(log_dir, root);
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_vec(log)?;
    let mut file = File::create(&tmp)?;
    file.write_all(&json)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&tmp, &path)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::categories::{Category, folder_name};
    use crate::mover::{ensure_folders, move_many};
    use std::fs;
    use tempfile::tempdir;

    fn planned(id: u32, src: &Path, root: &Path, name: &str) -> PlannedMove {
        PlannedMove {
            id,
            src: src.to_string_lossy().into_owned(),
            dest: root
                .join(folder_name(Category::Fotos))
                .join(name)
                .to_string_lossy()
                .into_owned(),
            category: Category::Fotos,
        }
    }

    fn ok_outcome(id: u32, dest: &Path) -> MoveOutcome {
        MoveOutcome {
            id,
            ok: true,
            dest: Some(dest.to_string_lossy().into_owned()),
            error: None,
        }
    }

    fn failed_outcome(id: u32) -> MoveOutcome {
        MoveOutcome {
            id,
            ok: false,
            dest: None,
            error: Some("in gebruik".to_string()),
        }
    }

    /// Een echte organize-run met één foto: log schrijven, verplaatsen, uitkomsten vastleggen.
    /// Geeft het oorspronkelijke pad en het pad in `Foto's` terug.
    fn organize_one_photo(log_dir: &Path, root: &Path) -> (PathBuf, PathBuf) {
        let src = root.join("a.jpg");
        fs::write(&src, b"foto").unwrap();
        let moves = vec![planned(1, &src, root, "a.jpg")];
        let created = ensure_folders(root, &[Category::Fotos]).unwrap();
        write_planned_in(log_dir, root, &moves, &created).unwrap();
        let outcomes = move_many(&moves);
        assert!(outcomes[0].ok, "{:?}", outcomes[0].error);
        apply_outcomes_in(log_dir, root, &outcomes).unwrap();
        (src, root.join("Foto's").join("a.jpg"))
    }

    /// Een echte organize-run met `n` foto's (`f0.jpg`, `f1.jpg`, ...), in volgorde van id.
    fn organize_many(log_dir: &Path, root: &Path, n: usize) -> Vec<(PathBuf, PathBuf)> {
        let mut moves = Vec::new();
        for i in 0..n {
            let name = format!("f{i}.jpg");
            let src = root.join(&name);
            fs::write(&src, name.as_bytes()).unwrap();
            moves.push(planned(u32::try_from(i).unwrap(), &src, root, &name));
        }
        let created = ensure_folders(root, &[Category::Fotos]).unwrap();
        write_planned_in(log_dir, root, &moves, &created).unwrap();
        let outcomes = move_many(&moves);
        assert!(outcomes.iter().all(|o| o.ok));
        apply_outcomes_in(log_dir, root, &outcomes).unwrap();
        moves
            .iter()
            .map(|m| {
                let name = Path::new(&m.src).file_name().unwrap();
                (PathBuf::from(&m.src), root.join("Foto's").join(name))
            })
            .collect()
    }

    /// Laat elke volgende `save` falen: de tijdelijke naam is bezet door een map.
    fn block_saves(log_dir: &Path, root: &Path) {
        fs::create_dir(log_path_in(log_dir, root).with_extension("json.tmp")).unwrap();
    }

    #[test]
    fn planned_then_outcomes_round_trip() {
        let log_dir = tempdir().unwrap();
        let root = tempdir().unwrap();
        let moves = vec![
            planned(1, &root.path().join("a.jpg"), root.path(), "a.jpg"),
            planned(2, &root.path().join("b.jpg"), root.path(), "b.jpg"),
        ];
        let created = vec![root.path().join("Foto's")];

        let written = write_planned_in(log_dir.path(), root.path(), &moves, &created).unwrap();
        assert_eq!(written.version, 1);
        assert_eq!(written.root, root.path().to_string_lossy());
        assert_eq!(
            written.folders_created,
            vec![created[0].to_string_lossy().into_owned()]
        );
        assert!(written.created_at_ms > 0);
        assert!(
            written
                .records
                .iter()
                .all(|r| r.status == RecordStatus::Planned)
        );

        let real_dest = root.path().join("Foto's").join("a (2).jpg");
        apply_outcomes_in(
            log_dir.path(),
            root.path(),
            &[ok_outcome(1, &real_dest), failed_outcome(2)],
        )
        .unwrap();

        let log = load_in(log_dir.path(), root.path()).unwrap().unwrap();
        assert_eq!(log.records.len(), 2);
        assert_eq!(log.records[0].id, 1);
        assert_eq!(log.records[0].status, RecordStatus::Moved);
        assert_eq!(log.records[0].dest, real_dest.to_string_lossy());
        assert_eq!(log.records[0].src, moves[0].src);
        assert_eq!(log.records[1].id, 2);
        assert_eq!(log.records[1].status, RecordStatus::Failed);
        assert_eq!(log.records[1].dest, moves[1].dest);
        assert_eq!(log.created_at_ms, written.created_at_ms);

        let info = info_in(log_dir.path(), root.path()).unwrap();
        assert_eq!(info.count, 1);
        assert_eq!(info.created_at_ms, written.created_at_ms);
    }

    #[test]
    fn log_is_written_atomically_under_a_hash_of_the_lowercased_root() {
        let log_dir = tempdir().unwrap();
        let root = tempdir().unwrap();

        write_planned_in(log_dir.path(), root.path(), &[], &[]).unwrap();

        let path = log_path_in(log_dir.path(), root.path());
        assert!(path.is_file());
        assert_eq!(path.parent(), Some(log_dir.path()));
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let upper = log_path_in(
            log_dir.path(),
            Path::new(&root.path().to_string_lossy().to_uppercase()),
        );
        assert_eq!(path, upper);
        let lowered = root.path().to_string_lossy().to_lowercase();
        assert_eq!(
            name,
            format!("{}.json", blake3::hash(lowered.as_bytes()).to_hex())
        );
        let other = tempdir().unwrap();
        assert_ne!(path, log_path_in(log_dir.path(), other.path()));
        let leftovers: Vec<_> = fs::read_dir(log_dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(leftovers.len(), 1, "{leftovers:?}");
    }

    #[test]
    fn write_planned_replaces_the_previous_log() {
        let log_dir = tempdir().unwrap();
        let root = tempdir().unwrap();
        organize_one_photo(log_dir.path(), root.path());

        let fresh = write_planned_in(log_dir.path(), root.path(), &[], &[]).unwrap();

        assert!(fresh.records.is_empty());
        let log = load_in(log_dir.path(), root.path()).unwrap().unwrap();
        assert!(log.records.is_empty());
        assert_eq!(info_in(log_dir.path(), root.path()).unwrap().count, 0);
    }

    #[test]
    fn restore_moves_files_back_and_removes_created_empty_folders() {
        let log_dir = tempdir().unwrap();
        let root = tempdir().unwrap();
        let (src, dest) = organize_one_photo(log_dir.path(), root.path());
        assert!(dest.exists() && !src.exists());

        let result = restore_in(log_dir.path(), root.path(), true).unwrap();

        assert_eq!(result.restored, 1);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(fs::read(&src).unwrap(), b"foto");
        assert!(!dest.exists());
        assert!(!root.path().join("Foto's").exists());
        let log = load_in(log_dir.path(), root.path()).unwrap().unwrap();
        assert_eq!(log.records[0].status, RecordStatus::Restored);
        assert_eq!(info_in(log_dir.path(), root.path()).unwrap().count, 0);
    }

    #[test]
    fn disorganize_keeps_folders() {
        let log_dir = tempdir().unwrap();
        let root = tempdir().unwrap();
        let (src, dest) = organize_one_photo(log_dir.path(), root.path());

        let result = restore_in(log_dir.path(), root.path(), false).unwrap();

        assert_eq!(result.restored, 1);
        assert_eq!(fs::read(&src).unwrap(), b"foto");
        assert!(!dest.exists());
        assert!(root.path().join("Foto's").is_dir());
        assert_eq!(info_in(log_dir.path(), root.path()).unwrap().count, 0);
    }

    #[test]
    fn undo_uses_unique_name_when_source_exists_again() {
        let log_dir = tempdir().unwrap();
        let root = tempdir().unwrap();
        let (src, _) = organize_one_photo(log_dir.path(), root.path());
        fs::write(&src, b"nieuwe inhoud").unwrap();

        let result = restore_in(log_dir.path(), root.path(), true).unwrap();

        assert_eq!(result.restored, 1);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(fs::read(&src).unwrap(), b"nieuwe inhoud");
        assert_eq!(fs::read(root.path().join("a (2).jpg")).unwrap(), b"foto");
        assert!(!root.path().join("Foto's").exists());
    }

    #[test]
    fn undo_keeps_a_created_folder_that_is_no_longer_empty() {
        let log_dir = tempdir().unwrap();
        let root = tempdir().unwrap();
        let (src, _) = organize_one_photo(log_dir.path(), root.path());
        let intruder = root.path().join("Foto's").join("later-erbij.jpg");
        fs::write(&intruder, b"van de gebruiker").unwrap();

        let result = restore_in(log_dir.path(), root.path(), true).unwrap();

        assert_eq!(result.restored, 1);
        assert!(src.exists());
        assert_eq!(fs::read(&intruder).unwrap(), b"van de gebruiker");
    }

    #[test]
    fn failed_restore_is_reported_and_stays_moved() {
        let log_dir = tempdir().unwrap();
        let root = tempdir().unwrap();
        let (src, dest) = organize_one_photo(log_dir.path(), root.path());
        fs::remove_file(&dest).unwrap();

        let result = restore_in(log_dir.path(), root.path(), true).unwrap();

        assert_eq!(result.restored, 0);
        assert_eq!(result.errors.len(), 1);
        assert_eq!((result.errors[0].id, result.errors[0].ok), (1, false));
        assert!(
            result.errors[0]
                .error
                .as_deref()
                .is_some_and(|e| !e.is_empty())
        );
        assert!(!src.exists());
        let log = load_in(log_dir.path(), root.path()).unwrap().unwrap();
        assert_eq!(log.records[0].status, RecordStatus::Moved);
        assert_eq!(info_in(log_dir.path(), root.path()).unwrap().count, 1);
    }

    #[test]
    fn restore_only_touches_moved_records() {
        let log_dir = tempdir().unwrap();
        let root = tempdir().unwrap();
        let staying = root.path().join("blijft.jpg");
        fs::write(&staying, b"onaangeroerd").unwrap();
        let src = root.path().join("a.jpg");
        fs::write(&src, b"foto").unwrap();
        let moves = vec![
            planned(1, &src, root.path(), "a.jpg"),
            planned(2, &staying, root.path(), "blijft.jpg"),
        ];
        let created = ensure_folders(root.path(), &[Category::Fotos]).unwrap();
        write_planned_in(log_dir.path(), root.path(), &moves, &created).unwrap();
        let first = move_many(&moves[..1]);
        apply_outcomes_in(
            log_dir.path(),
            root.path(),
            &[first[0].clone(), failed_outcome(2)],
        )
        .unwrap();

        let result = restore_in(log_dir.path(), root.path(), true).unwrap();

        assert_eq!(result.restored, 1);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(fs::read(&staying).unwrap(), b"onaangeroerd");
        let log = load_in(log_dir.path(), root.path()).unwrap().unwrap();
        assert_eq!(log.records[0].status, RecordStatus::Restored);
        assert_eq!(log.records[1].status, RecordStatus::Failed);
    }

    #[test]
    fn restore_with_checkpoints_restores_every_file_and_saves_the_log() {
        let log_dir = tempdir().unwrap();
        let root = tempdir().unwrap();
        let files = organize_many(log_dir.path(), root.path(), 2 * CHECKPOINT_EVERY + 20);

        let result = restore_in(log_dir.path(), root.path(), true).unwrap();

        assert_eq!(result.restored as usize, files.len());
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(
            files
                .iter()
                .all(|(src, dest)| src.exists() && !dest.exists())
        );
        let log = load_in(log_dir.path(), root.path()).unwrap().unwrap();
        assert!(
            log.records
                .iter()
                .all(|r| r.status == RecordStatus::Restored)
        );
        assert_eq!(info_in(log_dir.path(), root.path()).unwrap().count, 0);
    }

    #[test]
    fn failing_final_save_still_reports_the_restored_files() {
        let log_dir = tempdir().unwrap();
        let root = tempdir().unwrap();
        let files = organize_many(log_dir.path(), root.path(), 3);
        block_saves(log_dir.path(), root.path());

        let result = restore_in(log_dir.path(), root.path(), true).unwrap();

        assert_eq!(result.restored, 3);
        assert_eq!(result.errors.len(), 1, "{:?}", result.errors);
        assert_eq!(result.errors[0].id, u32::MAX);
        assert!(!result.errors[0].ok);
        assert!(
            result.errors[0]
                .error
                .as_deref()
                .is_some_and(|e| !e.is_empty())
        );
        assert!(
            files
                .iter()
                .all(|(src, dest)| src.exists() && !dest.exists())
        );
    }

    #[test]
    fn failing_checkpoint_stops_restoring_and_reports_the_save_error() {
        let log_dir = tempdir().unwrap();
        let root = tempdir().unwrap();
        let files = organize_many(log_dir.path(), root.path(), CHECKPOINT_EVERY + 20);
        block_saves(log_dir.path(), root.path());

        let result = restore_in(log_dir.path(), root.path(), true).unwrap();

        assert_eq!(result.restored as usize, CHECKPOINT_EVERY);
        assert_eq!(result.errors.len(), 1, "{:?}", result.errors);
        assert_eq!(result.errors[0].id, u32::MAX);
        for (i, (src, dest)) in files.iter().enumerate() {
            let done = i < CHECKPOINT_EVERY;
            assert_eq!((src.exists(), dest.exists()), (done, !done), "bestand {i}");
        }
    }

    #[test]
    fn restore_refuses_records_that_point_outside_the_root() {
        let log_dir = tempdir().unwrap();
        let root = tempdir().unwrap();
        let outside = tempdir().unwrap();
        let (_, dest) = organize_one_photo(log_dir.path(), root.path());
        let secret = outside.path().join("geheim.pdf");
        fs::write(&secret, b"niet aanraken").unwrap();
        let mut log = load_in(log_dir.path(), root.path()).unwrap().unwrap();
        // Record 1: terugzetten naar een plek buiten de map. Record 2: een bestand van buiten de map binnenhalen.
        log.records[0].src = outside.path().join("a.jpg").to_string_lossy().into_owned();
        log.records.push(UndoRecord {
            id: 2,
            src: root
                .path()
                .join("binnen.pdf")
                .to_string_lossy()
                .into_owned(),
            dest: secret.to_string_lossy().into_owned(),
            status: RecordStatus::Moved,
        });
        save(log_dir.path(), root.path(), &log).unwrap();

        let result = restore_in(log_dir.path(), root.path(), true).unwrap();

        assert_eq!(result.restored, 0);
        let ids: Vec<_> = result.errors.iter().map(|e| (e.id, e.ok)).collect();
        assert_eq!(ids, vec![(1, false), (2, false)]);
        assert!(result.errors.iter().all(|e| {
            e.error
                .as_deref()
                .is_some_and(|m| m.contains("buiten de map"))
        }));
        assert_eq!(fs::read(&dest).unwrap(), b"foto");
        assert!(!outside.path().join("a.jpg").exists());
        assert_eq!(fs::read(&secret).unwrap(), b"niet aanraken");
        assert!(!root.path().join("binnen.pdf").exists());
        let log = load_in(log_dir.path(), root.path()).unwrap().unwrap();
        assert!(log.records.iter().all(|r| r.status == RecordStatus::Moved));
    }

    #[test]
    fn undo_never_removes_folders_outside_the_root_or_the_root_itself() {
        let log_dir = tempdir().unwrap();
        let root = tempdir().unwrap();
        let outside = tempdir().unwrap();
        let empty_outside = outside.path().join("leeg");
        fs::create_dir(&empty_outside).unwrap();
        let folders = vec![empty_outside.clone(), root.path().to_path_buf()];
        write_planned_in(log_dir.path(), root.path(), &[], &folders).unwrap();

        let result = restore_in(log_dir.path(), root.path(), true).unwrap();

        assert_eq!(result.restored, 0);
        assert!(empty_outside.is_dir());
        assert!(root.path().is_dir());
    }

    #[test]
    fn restore_without_log_is_noop() {
        let log_dir = tempdir().unwrap();
        let root = tempdir().unwrap();

        let result = restore_in(log_dir.path(), root.path(), true).unwrap();

        assert_eq!(result.restored, 0);
        assert!(result.errors.is_empty());
        assert!(info_in(log_dir.path(), root.path()).is_none());
        assert!(load_in(log_dir.path(), root.path()).unwrap().is_none());
    }
}
