//! Re-export the Editor module components.
//!
//! This file serves as a compatibility layer for existing code.
//! New code should directly use the modular files from the editor directory.

pub use crate::editor::Editor;
pub use crate::editor::{EditorMode, EditorState};

// Re-export needed functionality from submodules
pub use crate::editor::commands::*;
pub use crate::editor::display::*;
pub use crate::editor::search::*;
pub use crate::editor::highlight::*;
