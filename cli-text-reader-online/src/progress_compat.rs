//! Re-export the Progress module components.
//!
//! This file serves as a compatibility layer for existing code.
//! New code should directly use the modular files from the progress directory.

// Re-export main functions
pub use crate::progress::generate_hash;
pub use crate::progress::events::{Progress, Event};
pub use crate::progress::local::{
    save_progress, load_progress,
    read_events_from_file, write_events_to_file
};
pub use crate::progress::highlights::{
    add_highlight, add_highlight_async,
    remove_highlight, remove_highlight_async,
    clear_highlights, clear_highlights_async,
    load_highlights, load_highlights_async,
    export_highlights, export_highlights_async
};
pub use crate::progress::server::{
    add_highlight_to_server,
    remove_highlight_from_server,
    clear_highlights_on_server,
    load_highlights_from_server,
    undo_last_highlight_on_server
};

// Define our own wrappers for undo functionality since they're missing from the modules
pub async fn undo_last_highlight_async(
    document_hash: u64,
    file_path: &str,
    client: Option<std::sync::Arc<crate::server::HyggClient>>,
) -> Result<bool, Box<dyn std::error::Error>> {
    if let Some(client) = client {
        return crate::progress::server::undo_last_highlight_on_server(document_hash, client).await;
    }
    // If no client, delegate to the synchronous version
    undo_last_highlight(document_hash, file_path)
}

pub fn undo_last_highlight(
    document_hash: u64,
    _file_path: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    use crate::progress::events::Event;
    use crate::progress::local::{read_events_from_file, write_events_to_file};
    
    // Read all events from the progress file
    let mut events = read_events_from_file().unwrap_or_default();
    
    // If no events, there's nothing to undo
    if events.is_empty() {
        return Ok(false);
    }
    
    // Track if we found a highlight event to undo
    let mut undone = false;
    
    // Iterate from the end to find the last highlight event for this document
    for i in (0..events.len()).rev() {
        match &events[i] {
            Event::AddHighlight { document_hash: hash, .. } | 
            Event::RemoveHighlight { document_hash: hash, .. } |
            Event::ClearHighlights { document_hash: hash, .. } => {
                if *hash == document_hash {
                    // Found a highlight event for this document, remove it
                    events.remove(i);
                    undone = true;
                    break;
                }
            },
            _ => continue, // Skip non-highlight events
        }
    }
    
    // If we found and removed an event, write the updated events back to the file
    if undone {
        write_events_to_file(&events)?;
    }
    
    Ok(undone)
}
