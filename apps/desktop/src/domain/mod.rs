//! Pure browser model: no UI, no IO. Everything here is deterministic and
//! covered by unit tests.

pub mod command;
pub mod layout;
pub mod omnibox;
pub mod search;
pub mod settings;
pub mod space;
pub mod tab;

pub use command::{validate_navigation, BrowserCommand, NavigationError, NavigationTarget};
pub use layout::{LayoutError, LayoutTree, PaneId, RectPx, SplitDirection};
pub use omnibox::{classify, resolve, OmniboxBuffer, OmniboxInput};
pub use search::SearchEngine;
pub use settings::{FeatureDisabledError, FeatureFlag, FeatureFlags, Settings, Theme};
pub use space::{SpaceError, SpaceId};
pub use tab::{Tab, TabId, TabStatus};
