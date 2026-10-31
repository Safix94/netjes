//! Het ongedaan-maken-log: per gekozen map één bestand in `%LOCALAPPDATA%\Netjes\undo`. Het is de
//! bron van waarheid voor "Ongedaan maken" en, in opnamemodus, voor "Rommelig maken". Terugzetten
//! gebruikt dezelfde no-replace-verplaatsing als het opruimen zelf: er wordt nooit iets overschreven
//! en nooit iets verwijderd, behalve lege mappen die Netjes zelf aanmaakte.

use crate::mover::{MoveOutcome, move_path_no_replace};
use crate::paths::app_data_dir;
use crate::planner::PlannedMove;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const LOG_VERSION: u32 = 1;
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
    let mut restored = 0;
    let mut errors = Vec::new();
    for record in log
        .records
        .iter_mut()
        .filter(|r| r.status == RecordStatus::Moved)
    {
        match move_path_no_replace(Path::new(&record.dest), Path::new(&record.src)) {
            Ok(_) => {
                record.status = RecordStatus::Restored;
                restored += 1;
            }
            Err(e) => errors.push(MoveOutcome {
                id: record.id,
                ok: false,
                dest: None,
                error: Some(e.to_string()),
            }),
        }
    }
    if remove_created_folders {
        for folder in &log.folders_created {
            // `remove_dir` weigert een map met inhoud: precies de bescherming die we willen.
            let _ = fs::remove_dir(folder);
        }
    }
    save(log_dir, root, &log)?;
    Ok(UndoResult { restored, errors })
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
        assert_eq!(name.len(), 64 + ".json".len());
        assert!(name.ends_with(".json"));
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
