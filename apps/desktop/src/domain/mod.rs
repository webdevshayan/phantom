//! Pure browser model: no UI, no IO. Everything here is deterministic and
//! covered by unit tests.

pub mod space;
pub mod tab;

pub use space::{SpaceError, SpaceId};
pub use tab::{Tab, TabId, TabStatus};
