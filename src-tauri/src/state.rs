//! De gedeelde toestand van de app: het laatste scanresultaat, het plan van het lopende
//! opruimen en een slot dat alles serialiseert wat bestanden verplaatst of het log schrijft.

use crate::planner::PlannedMove;
use crate::scan::ScanResult;
use crate::undo;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};

#[derive(Default)]
pub struct AppState {
    /// De laatste scan; `root` is de enige map waarvoor een opruimronde mag starten.
    pub last_scan: Mutex<Option<ScanResult>>,
    /// Het plan van `begin_organize`. Enkel wat hier in staat mag `move_files` verplaatsen.
    pub plan: Mutex<Option<Vec<PlannedMove>>>,
    /// Gehouden door alles wat bestanden verplaatst of het ongedaan-maken-log schrijft, zodat
    /// twee rondes elkaar nooit overlappen. Altijd nemen vóór `last_scan` en `plan`.
    pub organize_lock: Mutex<()>,
    /// Een eigen logmap (tests). `None` is de echte map in de app-gegevens.
    pub log_dir: Option<PathBuf>,
}

impl AppState {
    pub fn log_dir(&self) -> PathBuf {
        self.log_dir.clone().unwrap_or_else(undo::default_log_dir)
    }
}

/// Een mutex waarvan een vorige houder gepanikeerd is, blijft bruikbaar: de gegevens erin zijn
/// altijd in één keer vervangen, dus nooit half bijgewerkt.
pub fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
