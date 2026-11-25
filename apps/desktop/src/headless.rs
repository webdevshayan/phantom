//! Headless smoke path: opens the store, restores the session, prints a
//! summary, exits. Used by `phantom --headless` to verify persistence and
//! session integrity without any toolkit on the machine.

use std::path::Path;

use crate::browser::PhantomSession;
use crate::domain::settings::Settings;
use crate::infra::db::Store;
use crate::infra::paths;

/// Runs the headless pass, returning a process exit code.
pub fn run(profile: &Path) -> i32 {
    let Ok(db_path) = paths::ensure_profile_dirs(profile) else {
        eprintln!(
            "phantom: could not create the profile directory at {}",
            profile.display()
        );
        return 1;
    };
    let store = match Store::open(&db_path) {
        Ok(store) => store,
        Err(error) => {
            eprintln!("phantom: could not open {}: {error}", db_path.display());
            return 1;
        }
    };
    let settings = settings_load(&store);
    let mut session = PhantomSession::new(store, settings);
    let restored = match session.restore() {
        Ok(effects) => effects.len(),
        Err(error) => {
            eprintln!("phantom: session restore failed: {error}");
            return 1;
        }
    };
    let window_count = session.windows().len();
    let tab_count: usize = session.windows().iter().map(|w| w.tabs.len()).sum();
    let pane_count: usize = session.windows().iter().map(|w| w.pane_count()).sum();
    let settings_ref = session.settings();
    println!(
        "phantom {} headless: profile={} restored_windows={window_count} restored_tabs={tab_count} restored_panes={pane_count} restore_effects={restored} theme={:?} engine={:?} tiling={} drag_tabs={}",
        env!("CARGO_PKG_VERSION"),
        profile.display(),
        settings_ref.theme,
        settings_ref.search_engine,
        settings_ref.flags.enable_tiling,
        settings_ref.flags.enable_drag_tabs,
    );
    0
}

fn settings_load(store: &Store) -> Settings {
    crate::infra::settings_store::load_or_default(store).unwrap_or_else(|_| Settings::default())
}
