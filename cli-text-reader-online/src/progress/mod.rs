use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use crate::server::HyggClient;

pub mod events;
pub mod highlights;
pub mod local;
pub mod server;

// Public exports from submodules
pub use events::{Progress, Event};
pub use highlights::{
    add_highlight, add_highlight_async,
    remove_highlight, remove_highlight_async,
    clear_highlights, clear_highlights_async,
    load_highlights, load_highlights_async,
    export_highlights, export_highlights_async
};
pub use local::{save_progress, load_progress};

// Core utility functions
pub fn generate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}
