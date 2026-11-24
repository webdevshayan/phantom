use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::domain::space::SpaceId;

/// Resolves the profile directory that holds the state database and the
/// per-space webview data directories.
///
/// `PHANTOM_PROFILE_DIR` wins so QA can point Phantom at a scratch
/// directory; otherwise the platform default applies (`%APPDATA%\Phantom`
/// on Windows, `$XDG_DATA_HOME/phantom` or `$HOME/.local/share/phantom`).
pub fn profile_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("PHANTOM_PROFILE_DIR") {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    default_profile_dir_from(
        std::env::var_os("APPDATA").as_deref(),
        std::env::var_os("XDG_DATA_HOME").as_deref(),
        std::env::var_os("HOME").as_deref(),
    )
}

fn default_profile_dir_from(
    appdata: Option<&OsStr>,
    xdg_data_home: Option<&OsStr>,
    home: Option<&OsStr>,
) -> PathBuf {
    #[cfg(windows)]
    {
        let _ = (xdg_data_home, home);
        appdata
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("Phantom")
    }
    #[cfg(not(windows))]
    {
        let _ = appdata;
        let base = match xdg_data_home {
            Some(dir) if !dir.is_empty() => PathBuf::from(dir),
            _ => {
                let mut path = home
                    .filter(|value| !value.is_empty())
                    .map(PathBuf::from)
                    .unwrap_or_else(std::env::temp_dir);
                path.push(".local");
                path.push("share");
                path
            }
        };
        base.join("phantom")
    }
}

/// Path of the session state database inside the profile.
pub fn database_path(profile: &Path) -> PathBuf {
    profile.join("phantom.db")
}

/// Root of the per-space webview data directories.
pub fn spaces_root(profile: &Path) -> PathBuf {
    profile.join("spaces")
}

/// Data directory backing one space's webviews. `SpaceId` parsing is strict,
/// so the joined name can never traverse out of `spaces_root`.
pub fn space_data_dir(profile: &Path, space: &SpaceId) -> PathBuf {
    spaces_root(profile).join(space.as_str())
}

/// Creates the profile layout and returns the database path.
pub fn ensure_profile_dirs(profile: &Path) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(profile)?;
    std::fs::create_dir_all(spaces_root(profile))?;
    Ok(database_path(profile))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xdg_data_home_is_honored() {
        let dir = default_profile_dir_from(None, Some(OsStr::new("/tmp/xdg")), Some(OsStr::new("/home/z")));
        assert_eq!(dir, PathBuf::from("/tmp/xdg/phantom"));
    }

    #[test]
    fn empty_xdg_falls_back_to_home_layout() {
        let dir = default_profile_dir_from(None, Some(OsStr::new("")), Some(OsStr::new("/home/z")));
        assert_eq!(dir, PathBuf::from("/home/z/.local/share/phantom"));
    }

    #[test]
    fn missing_everything_lands_in_temp_dir() {
        let dir = default_profile_dir_from(None, None, None);
        assert!(dir.starts_with(std::env::temp_dir()));
        assert!(dir.ends_with("phantom"));
    }

    #[test]
    fn space_dir_joins_inside_spaces_root() {
        let profile = Path::new("/profile");
        let space = SpaceId::parse("Work-1").unwrap();
        let dir = space_data_dir(profile, &space);
        assert_eq!(dir, PathBuf::from("/profile/spaces/work-1"));
        assert!(dir.starts_with(spaces_root(profile)));
    }

    #[test]
    fn ensure_profile_dirs_creates_layout() {
        let profile = tempfile::tempdir().unwrap().path().join("nested");
        let db = ensure_profile_dirs(&profile).unwrap();
        assert_eq!(db, database_path(&profile));
        assert!(profile.is_dir());
        assert!(spaces_root(&profile).is_dir());
    }
}
