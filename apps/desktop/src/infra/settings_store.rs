use crate::domain::settings::{Settings, SETTINGS_SCHEMA_VERSION};
use crate::infra::db::{DbError, Store};

pub const SETTINGS_KEY: &str = "settings";

/// Loads persisted settings, falling back to defaults when nothing is stored
/// yet, the blob is unreadable, or it was written by a newer schema.
pub fn load_or_default(store: &Store) -> Result<Settings, DbError> {
    match store.meta(SETTINGS_KEY)? {
        Some(json) => Ok(Settings::from_stored(&json)
            .map_err(|_| DbError::Corrupt("settings json does not parse"))?),
        None => Ok(Settings::default()),
    }
}

/// Persists settings, stamped with the current schema version.
pub fn save(store: &mut Store, settings: &Settings) -> Result<(), DbError> {
    let stamped = Settings {
        schema_version: SETTINGS_SCHEMA_VERSION,
        ..settings.clone()
    };
    let json = serde_json::to_string(&stamped)
        .map_err(|_| DbError::Corrupt("settings could not be serialized"))?;
    store.set_meta(SETTINGS_KEY, &json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::search::SearchEngine;
    use crate::domain::settings::{FeatureFlag, Theme};

    #[test]
    fn roundtrip_preserves_values() {
        let mut store = Store::open_in_memory().unwrap();
        let mut settings = Settings::default();
        settings.theme = Theme::Dark;
        settings.search_engine = SearchEngine::Brave;
        settings.flags.enable_tiling = true;
        save(&mut store, &settings).unwrap();
        assert_eq!(load_or_default(&store).unwrap(), settings);
    }

    #[test]
    fn empty_store_yields_defaults() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(load_or_default(&store).unwrap(), Settings::default());
    }

    #[test]
    fn corrupt_blob_reports_corruption() {
        let mut store = Store::open_in_memory().unwrap();
        store.set_meta(SETTINGS_KEY, "{nope").unwrap();
        assert!(matches!(
            load_or_default(&store),
            Err(DbError::Corrupt("settings json does not parse"))
        ));
    }

    #[test]
    fn save_stamps_current_schema_version() {
        let mut store = Store::open_in_memory().unwrap();
        let mut settings = Settings::default();
        settings.schema_version = 99;
        save(&mut store, &settings).unwrap();
        let raw = store.meta(SETTINGS_KEY).unwrap().unwrap();
        assert!(raw.contains(&format!("\"schema_version\":{SETTINGS_SCHEMA_VERSION}")), "{raw}");
        // flags stay untouched by the stamping
        store.set_meta(SETTINGS_KEY, "{\"schema_version\":1,\"flags\":{\"enable_drag_tabs\":true}}").unwrap();
        let loaded = load_or_default(&store).unwrap();
        assert!(loaded.flags.enable_drag_tabs);
        assert_eq!(loaded.flags.get(FeatureFlag::EnableTiling), false);
    }
}
