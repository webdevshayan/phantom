//! Per-Space persistence. Each Space owns one directory under the data
//! root; the tab stack, preferences and form drafts are stored as
//! bincode blobs written atomically (temp file + rename), and a corrupt
//! or missing file falls back to the caller's default instead of
//! crashing. (Port of the old Hive repositories: the JSON box files
//! become bincode blobs with the same roles, still bound to exactly one
//! Space at a time and never shared between Spaces.)

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::form::FormSnapshot;
use crate::prefs::Preferences;
use crate::space::Space;
use crate::tab::TabStack;

const TAB_STACK_FILE: &str = "tab_stack.bin";
const PREFERENCES_FILE: &str = "preferences.bin";
const FORM_SNAPSHOTS_FILE: &str = "form_snapshots.bin";

pub struct SpaceStore {
    root: PathBuf,
}

impl SpaceStore {
    /// Opens (and creates) the Space's own data directory under
    /// `<data_root>/spaces/<segment>/`.
    pub fn open(data_root: &Path, space: &Space) -> io::Result<Self> {
        let root = data_root
            .join("spaces")
            .join(space.directory_segment());
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn load_tab_stack(&self) -> Option<TabStack> {
        read_blob(&self.root, TAB_STACK_FILE)
    }

    pub fn save_tab_stack(&self, stack: &TabStack) -> io::Result<()> {
        write_blob(&self.root, TAB_STACK_FILE, stack)
    }

    pub fn load_preferences(&self) -> Option<Preferences> {
        read_blob(&self.root, PREFERENCES_FILE)
    }

    pub fn save_preferences(&self, preferences: &Preferences) -> io::Result<()> {
        write_blob(&self.root, PREFERENCES_FILE, preferences)
    }

    pub fn load_form_snapshot(&self, page_url: &str) -> Option<FormSnapshot> {
        let stored: Vec<FormSnapshot> = read_blob(&self.root, FORM_SNAPSHOTS_FILE)?;
        stored.into_iter().find(|s| s.page_url == page_url)
    }

    /// Saves a draft, replacing any previous one for the same page.
    pub fn save_form_snapshot(&self, snapshot: FormSnapshot) -> io::Result<()> {
        let mut stored: Vec<FormSnapshot> =
            read_blob(&self.root, FORM_SNAPSHOTS_FILE).unwrap_or_default();
        match stored.iter_mut().find(|s| s.page_url == snapshot.page_url) {
            Some(slot) => *slot = snapshot,
            None => stored.push(snapshot),
        }
        write_blob(&self.root, FORM_SNAPSHOTS_FILE, &stored)
    }

    pub fn clear_form_snapshot(&self, page_url: &str) -> io::Result<()> {
        let mut stored: Vec<FormSnapshot> =
            read_blob(&self.root, FORM_SNAPSHOTS_FILE).unwrap_or_default();
        let before = stored.len();
        stored.retain(|s| s.page_url != page_url);
        if stored.len() == before {
            return Ok(());
        }
        write_blob(&self.root, FORM_SNAPSHOTS_FILE, &stored)
    }
}

fn read_blob<T: DeserializeOwned>(dir: &Path, name: &str) -> Option<T> {
    let bytes = fs::read(dir.join(name)).ok()?;
    bincode::deserialize(&bytes).ok()
}

fn write_blob<T: Serialize>(dir: &Path, name: &str, value: &T) -> io::Result<()> {
    let bytes = bincode::serialize(value).expect("blob encoding is infallible for these types");
    let temp = dir.join(format!("{name}.tmp"));
    fs::write(&temp, &bytes)?;
    fs::rename(&temp, dir.join(name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prefs::FeatureFlags;
    use crate::search::SearchEngineId;
    use crate::theme::ThemePreference;

    fn temp_root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("phantom-store-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn prefs() -> Preferences {
        Preferences {
            theme: ThemePreference::Dark,
            search_engine: SearchEngineId::Startpage,
            flags: FeatureFlags::ALL_OFF,
        }
    }

    #[test]
    fn blobs_round_trip_per_space() {
        let root = temp_root("round-trip");
        let space = Space::session();
        let store = SpaceStore::open(&root, &space).expect("open");
        assert_eq!(store.root(), root.join("spaces").join("session"));

        let stack = TabStack::initial("session");
        assert!(store.load_tab_stack().is_none());
        store.save_tab_stack(&stack).expect("save");
        assert_eq!(store.load_tab_stack(), Some(stack));

        assert!(store.load_preferences().is_none());
        store.save_preferences(&prefs()).expect("save");
        assert_eq!(store.load_preferences(), Some(prefs()));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn spaces_do_not_share_directories() {
        let root = temp_root("isolation");
        let session = SpaceStore::open(&root, &Space::session()).expect("open session");
        let other_space = Space {
            id: "work".to_string(),
            label: "Work".to_string(),
        };
        let work = SpaceStore::open(&root, &other_space).expect("open work");
        session
            .save_preferences(&prefs())
            .expect("save in session");
        assert_eq!(work.load_preferences(), None, "work must not read session");
        assert_eq!(session.load_preferences(), Some(prefs()));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn form_snapshots_upsert_and_clear() {
        let root = temp_root("forms");
        let store = SpaceStore::open(&root, &Space::session()).expect("open");
        let url = "https://example.com/login";
        let draft = FormSnapshot {
            page_url: url.to_string(),
            fields: vec![("user".to_string(), "ada".to_string())],
        };
        store.save_form_snapshot(draft.clone()).expect("save");
        let updated = FormSnapshot {
            page_url: url.to_string(),
            fields: vec![("user".to_string(), "grace".to_string())],
        };
        store.save_form_snapshot(updated.clone()).expect("upsert");
        assert_eq!(store.load_form_snapshot(url), Some(updated));
        store.clear_form_snapshot(url).expect("clear");
        assert_eq!(store.load_form_snapshot(url), None);
        store.clear_form_snapshot(url).expect("clear is idempotent");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn corrupt_blobs_fall_back_to_none() {
        let root = temp_root("corrupt");
        let store = SpaceStore::open(&root, &Space::session()).expect("open");
        fs::write(store.root().join(TAB_STACK_FILE), b"not bincode at all")
            .expect("plant garbage");
        assert_eq!(store.load_tab_stack(), None);
        let _ = fs::remove_dir_all(&root);
    }
}
