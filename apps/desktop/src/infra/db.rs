use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::domain::layout::LayoutTree;
use crate::domain::settings::Settings;
use crate::domain::space::SpaceId;
use crate::domain::tab::TabId;

/// Latest applied schema version; migrations are plain SQL applied in order,
/// each guarded by `PRAGMA user_version` progress tracking.
const MIGRATIONS: &[&str] = &[
    // v1: meta key-value store, windows with geometry + layout, tabs in strip order
    "CREATE TABLE meta (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );
    CREATE TABLE windows (
        id INTEGER PRIMARY KEY,
        x REAL,
        y REAL,
        width REAL,
        height REAL,
        maximized INTEGER NOT NULL DEFAULT 0,
        active_tab_id INTEGER,
        layout_json TEXT NOT NULL
    );
    CREATE TABLE tabs (
        id INTEGER PRIMARY KEY,
        window_id INTEGER NOT NULL REFERENCES windows(id) ON DELETE CASCADE,
        space_id TEXT NOT NULL,
        title TEXT NOT NULL,
        url TEXT,
        position INTEGER NOT NULL
    );
    CREATE INDEX idx_tabs_window_position ON tabs(window_id, position);",
];

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("database: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("session record is corrupt: {0}")]
    Corrupt(&'static str),
}

/// Window geometry as reported by the OS. `None` fields fall back to the
/// platform default placement on restore.
#[derive(Clone, Copy, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct WindowBounds {
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub width: Option<f64>,
    pub height: Option<f64>,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct TabSnapshot {
    pub id: TabId,
    pub space: SpaceId,
    pub title: String,
    pub url: Option<String>,
}

/// One persisted window: geometry, strip order and the split layout. The
/// layout's pane references must resolve against `tabs` (validated on load).
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct WindowSnapshot {
    pub id: u64,
    pub bounds: WindowBounds,
    pub maximized: bool,
    pub active_tab: Option<TabId>,
    pub layout: LayoutTree,
    pub tabs: Vec<TabSnapshot>,
}

impl WindowSnapshot {
    /// Consistency checks applied before a snapshot is trusted.
    pub fn validate(&self) -> Result<(), DbError> {
        let known: std::collections::HashSet<u64> = self.tabs.iter().map(|t| t.id.0).collect();
        for tab in self.layout.tab_ids() {
            if !known.contains(&tab.0) {
                return Err(DbError::Corrupt("layout references an unknown tab"));
            }
        }
        if let Some(active) = self.active_tab {
            if !known.contains(&active.0) {
                return Err(DbError::Corrupt("active tab is unknown"));
            }
        }
        Ok(())
    }
}

/// Everything restored at launch.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct SessionSnapshot {
    pub windows: Vec<WindowSnapshot>,
}

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: &Path) -> Result<Store, DbError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        finish_open(Connection::open(path)?)
    }

    /// In-memory store for tests and the headless smoke path.
    pub fn open_in_memory() -> Result<Store, DbError> {
        finish_open(Connection::open_in_memory()?)
    }

    pub fn applied_schema_version(&self) -> i64 {
        self.conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap_or(0)
    }

    fn migrate(conn: &Connection) -> Result<(), DbError> {
        let current: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        for (index, sql) in MIGRATIONS.iter().enumerate() {
            let version = (index + 1) as i64;
            if version <= current {
                continue;
            }
            conn.execute_batch(sql)?;
            conn.pragma_update(None, "user_version", version)?;
        }
        Ok(())
    }

    // meta ----

    pub fn set_meta(&mut self, key: &str, value: &str) -> Result<(), DbError> {
        self.conn.execute(
            "INSERT INTO meta(key, value) VALUES(?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn meta(&self, key: &str) -> Result<Option<String>, DbError> {
        let value = self
            .conn
            .query_row(
                "SELECT value FROM meta WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;
        Ok(value)
    }

    // session ----

    /// Replaces the whole session atomically: windows, tabs, layouts.
    pub fn save_session(&mut self, session: &SessionSnapshot) -> Result<(), DbError> {
        let tx = self.conn.transaction()?;
        tx.execute_batch("DELETE FROM tabs; DELETE FROM windows;")?;
        for window in &session.windows {
            let layout_json = serde_json::to_string(&window.layout)
                .map_err(|_| DbError::Corrupt("layout could not be serialized"))?;
            tx.execute(
                "INSERT INTO windows(id, x, y, width, height, maximized, active_tab_id, layout_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    window.id as i64,
                    window.bounds.x,
                    window.bounds.y,
                    window.bounds.width,
                    window.bounds.height,
                    window.maximized as i64,
                    window.active_tab.map(|t| t.0 as i64),
                    layout_json,
                ],
            )?;
            for (position, tab) in window.tabs.iter().enumerate() {
                tx.execute(
                    "INSERT INTO tabs(id, window_id, space_id, title, url, position)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        tab.id.0 as i64,
                        window.id as i64,
                        tab.space.as_str(),
                        tab.title,
                        tab.url,
                        position as i64,
                    ],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn load_session(&self) -> Result<SessionSnapshot, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, x, y, width, height, maximized, active_tab_id, layout_json
             FROM windows ORDER BY id",
        )?;
        let mut rows = stmt.query([])?;
        let mut windows = Vec::new();
        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            let active_tab_id: Option<i64> = row.get(6)?;
            let layout_json: String = row.get(7)?;
            let layout: LayoutTree = serde_json::from_str(&layout_json)
                .map_err(|_| DbError::Corrupt("layout json does not parse"))?;
            let maximized: i64 = row.get(5)?;
            windows.push(WindowSnapshot {
                id: id as u64,
                bounds: WindowBounds {
                    x: row.get(1)?,
                    y: row.get(2)?,
                    width: row.get(3)?,
                    height: row.get(4)?,
                },
                maximized: maximized != 0,
                active_tab: active_tab_id.map(|v| TabId(v as u64)),
                layout,
                tabs: self.load_tabs(id)?,
            });
        }
        Ok(SessionSnapshot { windows })
    }

    pub fn clear_session(&mut self) -> Result<(), DbError> {
        self.conn
            .execute_batch("DELETE FROM tabs; DELETE FROM windows;")?;
        Ok(())
    }

    fn load_tabs(&self, window_id: i64) -> Result<Vec<TabSnapshot>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, space_id, title, url FROM tabs WHERE window_id = ?1 ORDER BY position",
        )?;
        let rows = stmt.query_map(params![window_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })?;
        let mut tabs = Vec::new();
        for row in rows {
            let (id, space_raw, title, url) = row?;
            let space = SpaceId::parse(&space_raw)
                .map_err(|_| DbError::Corrupt("space id does not parse"))?;
            tabs.push(TabSnapshot {
                id: TabId(id as u64),
                space,
                title,
                url,
            });
        }
        Ok(tabs)
    }

    // settings blob ----

    pub fn save_settings(&mut self, settings: &Settings) -> Result<(), DbError> {
        let json = serde_json::to_string(settings)
            .map_err(|_| DbError::Corrupt("settings could not be serialized"))?;
        self.set_meta("settings", &json)
    }

    pub fn load_settings(&self) -> Result<Option<Settings>, DbError> {
        match self.meta("settings")? {
            Some(json) => {
                Ok(Some(Settings::from_stored(&json).map_err(|_| {
                    DbError::Corrupt("settings json does not parse")
                })?))
            }
            None => Ok(None),
        }
    }
}

fn finish_open(conn: Connection) -> Result<Store, DbError> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    Store::migrate(&conn)?;
    Ok(Store { conn })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::layout::{LayoutTree, PaneId};

    fn sample_window(id: u64) -> WindowSnapshot {
        let tab_id = TabId(id * 10);
        WindowSnapshot {
            id,
            bounds: WindowBounds {
                x: Some(10.0),
                y: Some(20.0),
                width: Some(1280.0),
                height: Some(800.0),
            },
            maximized: false,
            active_tab: Some(tab_id),
            layout: LayoutTree::pane(PaneId(id), tab_id),
            tabs: vec![TabSnapshot {
                id: tab_id,
                space: SpaceId::default_space(),
                title: "Example".into(),
                url: Some("https://example.com/".into()),
            }],
        }
    }

    #[test]
    fn migration_is_idempotent() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(store.applied_schema_version(), MIGRATIONS.len() as i64);
        // reopening the same connection path would skip already-applied versions;
        // verify by re-running migrate against the live connection
        Store::migrate(&store.conn).unwrap();
        assert_eq!(store.applied_schema_version(), MIGRATIONS.len() as i64);
    }

    #[test]
    fn session_roundtrip_preserves_everything() {
        let mut store = Store::open_in_memory().unwrap();
        let snapshot = SessionSnapshot {
            windows: vec![sample_window(1), sample_window(2)],
        };
        store.save_session(&snapshot).unwrap();
        let loaded = store.load_session().unwrap();
        assert_eq!(loaded, snapshot);
    }

    #[test]
    fn clear_session_empties_state() {
        let mut store = Store::open_in_memory().unwrap();
        store
            .save_session(&SessionSnapshot {
                windows: vec![sample_window(1)],
            })
            .unwrap();
        store.clear_session().unwrap();
        assert_eq!(store.load_session().unwrap().windows.len(), 0);
    }

    #[test]
    fn corrupt_layout_is_reported_not_panicked() {
        let store = Store::open_in_memory().unwrap();
        store
            .conn
            .execute_batch(
                "INSERT INTO windows(id, maximized, layout_json) VALUES (7, 0, 'not json')",
            )
            .unwrap();
        assert!(matches!(
            store.load_session(),
            Err(DbError::Corrupt("layout json does not parse"))
        ));
    }

    #[test]
    fn snapshot_referencing_unknown_tab_is_rejected() {
        let mut window = sample_window(1);
        window.layout = LayoutTree::pane(PaneId(1), TabId(404));
        assert!(window.validate().is_err());

        window.layout = LayoutTree::pane(PaneId(1), TabId(10));
        window.active_tab = Some(TabId(404));
        assert!(window.validate().is_err());

        window.active_tab = Some(TabId(10));
        assert!(window.validate().is_ok());
    }

    #[test]
    fn settings_blob_roundtrip() {
        let mut store = Store::open_in_memory().unwrap();
        assert!(store.load_settings().unwrap().is_none());
        let mut settings = Settings::default();
        settings.theme = crate::domain::settings::Theme::Dark;
        store.save_settings(&settings).unwrap();
        assert_eq!(store.load_settings().unwrap(), Some(settings));
    }

    #[test]
    fn file_store_persists_across_connections() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("phantom.db");
        {
            let mut store = Store::open(&path).unwrap();
            store
                .save_session(&SessionSnapshot {
                    windows: vec![sample_window(5)],
                })
                .unwrap();
        }
        let store = Store::open(&path).unwrap();
        let loaded = store.load_session().unwrap();
        assert_eq!(loaded.windows.len(), 1);
        assert_eq!(loaded.windows[0].id, 5);
    }
}
