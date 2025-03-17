use chrono::{DateTime, Utc};
use dirs::config_dir;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::fs::OpenOptions;
use std::hash::{Hash, Hasher};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::runtime::Runtime;
use crate::server::HyggClient;

#[derive(Serialize, Deserialize)]
pub struct Progress {
  pub document_hash: u64,
  pub offset: usize,
  pub total_lines: usize,
  pub percentage: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
enum Event {
  UpdateProgress {
    timestamp: DateTime<Utc>,
    document_hash: u64,
    offset: usize,
    total_lines: usize,
    percentage: f64,
  },
  AddHighlight {
    timestamp: DateTime<Utc>,
    document_hash: u64,
    line_number: usize,
  },
  RemoveHighlight {
    timestamp: DateTime<Utc>,
    document_hash: u64,
    line_number: usize,
  },
  ClearHighlights {
    timestamp: DateTime<Utc>,
    document_hash: u64,
  },
  UndoHighlight {
    timestamp: DateTime<Utc>,
    document_hash: u64,
  },
}

pub fn generate_hash<T: Hash>(t: &T) -> u64 {
  let mut s = DefaultHasher::new();
  t.hash(&mut s);
  s.finish()
}

fn get_progress_file_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
  let mut config_path =
    config_dir().ok_or("Unable to find config directory")?;
  config_path.push("hygg");
  std::fs::create_dir_all(&config_path)?;
  config_path.push(".progress.jsonl");
  Ok(config_path)
}

pub fn save_progress(
  document_hash: u64,
  offset: usize,
  total_lines: usize,
) -> Result<(), Box<dyn std::error::Error>> {
  let percentage = (offset as f64 / total_lines as f64) * 100.0;
  let event = Event::UpdateProgress {
    timestamp: Utc::now(),
    document_hash,
    offset,
    total_lines,
    percentage,
  };
  let serialized = serde_json::to_string(&event)?;
  let progress_file_path = get_progress_file_path()?;
  let mut file =
    OpenOptions::new().create(true).append(true).open(progress_file_path)?;
  file.write_all(serialized.as_bytes())?;
  file.write_all(b"\n")?;
  Ok(())
}

pub fn load_progress(
  document_hash: u64,
) -> Result<Progress, Box<dyn std::error::Error>> {
  let progress_file_path = get_progress_file_path()?;
  let file = OpenOptions::new().read(true).open(progress_file_path)?;
  let reader = io::BufReader::new(file);
  let mut latest_progress: Option<Progress> = None;

  for line in reader.lines() {
    let line = line?;
    let event: Event = serde_json::from_str(&line)?;
    if let Event::UpdateProgress {
      document_hash: hash,
      offset,
      total_lines,
      percentage,
      ..
    } = event
    {
      if hash == document_hash {
        latest_progress = Some(Progress {
          document_hash: hash,
          offset,
          total_lines,
          percentage,
        });
      }
    }
  }

  latest_progress
    .ok_or_else(|| "No progress found for the given document hash".into())
}

/// Add a highlight for a specific line in a document
/// 
/// This function handles both local and server-side highlights.
/// For server-side highlights, you should use the `add_highlight_async` function instead.
pub fn add_highlight(
  document_hash: u64,
  line_number: usize,
  file_path: &str,
  client: Option<Arc<HyggClient>>,
) -> Result<(), Box<dyn std::error::Error>> {
  // For server-side highlights, notify the user to use the async version
  if client.is_some() {
    println!("Error: Cannot call add_highlight with client from a synchronous context");
    println!("Use add_highlight_async instead");
    return Err("Cannot use server-side highlights from a synchronous context. Use add_highlight_async instead".into());
  }

  // Fallback to local storage for offline mode
  handle_local_add_highlight(document_hash, line_number)
}

/// Async version of add_highlight for server-side highlights
pub async fn add_highlight_async(
  document_hash: u64,
  line_number: usize,
  file_path: &str,
  client: Option<Arc<HyggClient>>,
) -> Result<(), Box<dyn std::error::Error>> {
  println!("add_highlight_async called for document_hash={}, line={}", document_hash, line_number);
  
  // If we have a client, use the server API
  if let Some(client) = client {
    println!("Adding highlight for line {} via server API", line_number);
    client.add_highlight(file_path, &document_hash.to_string(), line_number).await?;
    return Ok(());
  }
  
  // Fallback to local storage for offline mode
  handle_local_add_highlight(document_hash, line_number)
}

/// Helper function to handle local highlight addition
fn handle_local_add_highlight(document_hash: u64, line_number: usize) -> Result<(), Box<dyn std::error::Error>> {
  let event = Event::AddHighlight {
    timestamp: Utc::now(),
    document_hash,
    line_number,
  };
  let serialized = serde_json::to_string(&event)?;
  let progress_file_path = get_progress_file_path()?;
  let mut file =
    OpenOptions::new().create(true).append(true).open(progress_file_path)?;
  file.write_all(serialized.as_bytes())?;
  file.write_all(b"\n")?;
  Ok(())
}

/// Remove a highlight for a specific line in a document
/// 
/// This function handles both local and server-side highlights.
/// For server-side highlights, you should use the `remove_highlight_async` function instead.
pub fn remove_highlight(
  document_hash: u64,
  line_number: usize,
  file_path: &str,
  client: Option<Arc<HyggClient>>,
) -> Result<(), Box<dyn std::error::Error>> {
  // For server-side highlights, notify the user to use the async version
  if client.is_some() {
    println!("Error: Cannot call remove_highlight with client from a synchronous context");
    println!("Use remove_highlight_async instead");
    return Err("Cannot use server-side highlights from a synchronous context. Use remove_highlight_async instead".into());
  }

  // Fallback to local storage for offline mode
  handle_local_remove_highlight(document_hash, line_number)
}

/// Async version of remove_highlight for server-side highlights
pub async fn remove_highlight_async(
  document_hash: u64,
  line_number: usize,
  file_path: &str,
  client: Option<Arc<HyggClient>>,
) -> Result<(), Box<dyn std::error::Error>> {
  println!("remove_highlight_async called for document_hash={}, line={}", document_hash, line_number);
  
  // If we have a client, use the server API
  if let Some(client) = client {
    println!("Removing highlight for line {} via server API", line_number);
    client.remove_highlight(file_path, &document_hash.to_string(), line_number).await?;
    return Ok(());
  }
  
  // Fallback to local storage for offline mode
  handle_local_remove_highlight(document_hash, line_number)
}

/// Helper function to handle local highlight removal
fn handle_local_remove_highlight(document_hash: u64, line_number: usize) -> Result<(), Box<dyn std::error::Error>> {
  let event = Event::RemoveHighlight {
    timestamp: Utc::now(),
    document_hash,
    line_number,
  };
  let serialized = serde_json::to_string(&event)?;
  let progress_file_path = get_progress_file_path()?;
  let mut file =
    OpenOptions::new().create(true).append(true).open(progress_file_path)?;
  file.write_all(serialized.as_bytes())?;
  file.write_all(b"\n")?;
  Ok(())
}

/// Clear all highlights for a document
/// 
/// This function handles both local and server-side highlights.
/// For server-side highlights, you should use the `clear_highlights_async` function instead.
pub fn clear_highlights(
  document_hash: u64,
  client: Option<Arc<HyggClient>>,
) -> Result<(), Box<dyn std::error::Error>> {
  // For server-side highlights, notify the user to use the async version
  if client.is_some() {
    println!("Error: Cannot call clear_highlights with client from a synchronous context");
    println!("Use clear_highlights_async instead");
    return Err("Cannot use server-side highlights from a synchronous context. Use clear_highlights_async instead".into());
  }

  // Fallback to local storage for offline mode
  handle_local_clear_highlights(document_hash)
}

/// Async version of clear_highlights for server-side highlights
pub async fn clear_highlights_async(
  document_hash: u64,
  client: Option<Arc<HyggClient>>,
) -> Result<(), Box<dyn std::error::Error>> {
  println!("clear_highlights_async called for document_hash={}", document_hash);
  
  // If we have a client, use the server API
  if let Some(client) = client {
    println!("Clearing all highlights via server API");
    client.clear_highlights(&document_hash.to_string()).await?;
    return Ok(());
  }
  
  // Fallback to local storage for offline mode
  handle_local_clear_highlights(document_hash)
}

/// Helper function to handle local highlight clearing
fn handle_local_clear_highlights(document_hash: u64) -> Result<(), Box<dyn std::error::Error>> {
  let event = Event::ClearHighlights {
    timestamp: Utc::now(),
    document_hash,
  };
  let serialized = serde_json::to_string(&event)?;
  let progress_file_path = get_progress_file_path()?;
  let mut file =
    OpenOptions::new().create(true).append(true).open(progress_file_path)?;
  file.write_all(serialized.as_bytes())?;
  file.write_all(b"\n")?;
  Ok(())
}

/// Load all highlighted lines for a document
/// 
/// This function handles both local and server-side highlights.
/// For server-side highlights, you should call this in an async context
/// or use the `load_highlights_async` function instead.
pub fn load_highlights(
  document_hash: u64,
  client: Option<Arc<HyggClient>>,
) -> Result<Vec<usize>, Box<dyn std::error::Error>> {
  println!("Loading highlights for document hash: {}", document_hash);
  
  // For server-side highlights, delegate to the async version
  if client.is_some() {
    println!("Error: Cannot call load_highlights with client from a synchronous context");
    println!("Either call this function in an async context or use load_highlights_async instead");
    return Err("Cannot use server-side highlights from a synchronous context. Use load_highlights_async instead".into());
  }
  
  // Handle local storage fallback
  println!("Using local storage for highlight loading");
  handle_local_load_highlights(document_hash)
}

/// Async version of load_highlights for server-side highlights
pub async fn load_highlights_async(
  document_hash: u64,
  client: Option<Arc<HyggClient>>,
) -> Result<Vec<usize>, Box<dyn std::error::Error>> {
  println!("load_highlights_async called with document_hash={}", document_hash);
  
  if let Some(client) = client {
    println!("Loading highlights via server API");
    let server_highlights = client.get_highlights(&document_hash.to_string()).await?;
    
    // Extract just the line numbers
    let line_numbers: Vec<usize> = server_highlights.iter()
      .map(|h| h.line_number)
      .collect();
    
    println!("Received {} highlights from server", line_numbers.len());
    return Ok(line_numbers);
  }
  
  // Handle local storage fallback
  println!("Using local storage for highlight loading");
  handle_local_load_highlights(document_hash)
}

/// Helper function to handle local highlight loading logic
fn handle_local_load_highlights(document_hash: u64) -> Result<Vec<usize>, Box<dyn std::error::Error>> {
  let progress_file_path = get_progress_file_path()?;
  
  // Check if the progress file exists first
  if !progress_file_path.exists() {
    println!("No progress file found at: {:?}", progress_file_path);
    return Ok(Vec::new());
  }
  
  let file = match OpenOptions::new().read(true).open(&progress_file_path) {
    Ok(f) => f,
    Err(e) => {
      println!("Error opening progress file: {}", e);
      return Ok(Vec::new());
    }
  };
  
  let reader = io::BufReader::new(file);
  let mut highlights = Vec::new();
  let mut events_history: Vec<Event> = Vec::new();

  // First collect all highlight-related events for this document
  for line in reader.lines() {
    let line = match line {
      Ok(l) => l,
      Err(e) => {
        println!("Error reading line from progress file: {}", e);
        continue;
      }
    };
    
    let event: Event = match serde_json::from_str(&line) {
      Ok(e) => e,
      Err(e) => {
        println!("Error parsing event from line: {}", e);
        continue;
      }
    };
    
    match event {
      Event::AddHighlight { document_hash: hash, .. } |
      Event::RemoveHighlight { document_hash: hash, .. } |
      Event::ClearHighlights { document_hash: hash, .. } |
      Event::UndoHighlight { document_hash: hash, .. } => {
        if hash == document_hash {
          events_history.push(event);
        }
      },
      _ => {}
    }
  }
  
  println!("Found {} highlight events for document", events_history.len());
  
  // Process events in chronological order
  for (i, event) in events_history.iter().enumerate() {
    match event {
      Event::AddHighlight { line_number, .. } => {
        println!("  Event {}: Adding highlight for line {}", i, line_number);
        // Only add if not already highlighted
        if !highlights.contains(line_number) {
          highlights.push(*line_number);
        }
      },
      Event::RemoveHighlight { line_number, .. } => {
        println!("  Event {}: Removing highlight for line {}", i, line_number);
        // Remove this line number from highlights
        highlights.retain(|&l| l != *line_number);
      },
      Event::ClearHighlights { .. } => {
        println!("  Event {}: Clearing all highlights", i);
        highlights.clear();
      },
      Event::UndoHighlight { .. } => {
        println!("  Event {}: Undoing last highlight action", i);
        // Find the index of the current event we're processing
        if i > 0 {
          // Get the most recent highlight action before this undo
          let prev_event = &events_history[i-1];
          
          match prev_event {
            Event::AddHighlight { line_number, .. } => {
              // Undo an addition by removing the line
              println!("    Undoing addition of line {}", line_number);
              highlights.retain(|&l| l != *line_number);
            },
            Event::RemoveHighlight { line_number, .. } => {
              // Undo a removal by adding the line back
              println!("    Undoing removal of line {}", line_number);
              if !highlights.contains(line_number) {
                highlights.push(*line_number);
              }
            },
            Event::ClearHighlights { .. } => {
              println!("    Cannot fully undo a clear operation");
              // We can't restore previous highlights after a clear
              // as we don't store that full state
            },
            _ => {}
          }
        }
      },
      _ => {}
    }
  }
  
  println!("Final highlight count: {} lines", highlights.len());

  Ok(highlights)
}

/// Export all highlights for a document as text
/// 
/// This function handles both local and server-side highlights.
/// For server-side highlights, you should use the `export_highlights_async` function instead.
pub fn export_highlights(
  document_hash: u64,
  lines: &[String],
  client: Option<Arc<HyggClient>>,
) -> Result<String, Box<dyn std::error::Error>> {
  if client.is_some() {
    println!("Error: Cannot call export_highlights with client from a synchronous context");
    println!("Either call this function in an async context or use export_highlights_async instead");
    return Err("Cannot use server-side highlights from a synchronous context. Use export_highlights_async instead".into());
  }
  
  // Handle local storage
  println!("Using local storage for highlight export");
  let highlights = load_highlights(document_hash, None)?;
  format_highlights_result(highlights, lines)
}

/// Async version of export_highlights for server-side highlights
pub async fn export_highlights_async(
  document_hash: u64,
  lines: &[String],
  client: Option<Arc<HyggClient>>,
) -> Result<String, Box<dyn std::error::Error>> {
  let highlights = if client.is_some() {
    load_highlights_async(document_hash, client).await?
  } else {
    load_highlights(document_hash, None)?
  };
  
  format_highlights_result(highlights, lines)
}

/// Helper function to format highlights result
fn format_highlights_result(highlights: Vec<usize>, lines: &[String]) -> Result<String, Box<dyn std::error::Error>> {
  let mut result = String::new();
  
  for &line_num in &highlights {
    if line_num < lines.len() {
      result.push_str(&format!("Line {}: {}\n", line_num + 1, lines[line_num]));
    }
  }
  
  Ok(result)
}

/// Gets the last highlight event for a document
fn get_last_highlight_event(document_hash: u64) -> Result<Option<Event>, Box<dyn std::error::Error>> {
  let progress_file_path = get_progress_file_path()?;
  let file = OpenOptions::new().read(true).open(progress_file_path)?;
  let reader = io::BufReader::new(file);
  let mut last_event: Option<Event> = None;
  
  for line in reader.lines() {
    let line = line?;
    let event: Event = serde_json::from_str(&line)?;
    
    match event {
      Event::AddHighlight { document_hash: hash, .. } |
      Event::RemoveHighlight { document_hash: hash, .. } |
      Event::ClearHighlights { document_hash: hash, .. } |
      Event::UndoHighlight { document_hash: hash, .. } => {
        if hash == document_hash {
          // Keep track of the last highlight-related event
          last_event = Some(event);
        }
      },
      _ => {}
    }
  }
  
  Ok(last_event)
}

/// Undo the last highlight action for a document
/// 
/// This function handles both local and server-side highlight management.
/// For server-side highlights, you should call this in an async context
/// or use the `undo_last_highlight_async` function instead.
pub fn undo_last_highlight(
  document_hash: u64,
  file_path: &str,
  client: Option<Arc<HyggClient>>,
) -> Result<bool, Box<dyn std::error::Error>> {
  println!("undo_last_highlight called with document_hash={}", document_hash);
  
  // For server-side highlights, delegate to the async version
  if client.is_some() {
    println!("Error: Cannot call undo_last_highlight with client from a synchronous context");
    println!("Either call this function in an async context or use undo_last_highlight_async instead");
    return Err("Cannot use server-side highlights from a synchronous context. Use undo_last_highlight_async instead".into());
  }
  
  // For local storage fallback
  println!("Using local storage for highlight management");
  handle_local_undo_highlight(document_hash)
}

/// Async version of undo_last_highlight for server-side highlights
pub async fn undo_last_highlight_async(
  document_hash: u64,
  file_path: &str,
  client: Option<Arc<HyggClient>>,
) -> Result<bool, Box<dyn std::error::Error>> {
  println!("undo_last_highlight_async called with document_hash={}", document_hash);
  
  if let Some(client) = client {
    println!("Using server-based highlight management through HyggClient");
    
    // Call the server-side API directly in an async context
    println!("Undoing last highlight action via server API");
    let hash_str = document_hash.to_string();
    println!("Calling client.undo_last_highlight with hash={}", hash_str);
    
    match client.undo_last_highlight(&hash_str).await {
      Ok(result) => {
        println!("Server returned was_undone: {}", result);
        Ok(result)
      },
      Err(e) => {
        println!("Error in undo_last_highlight: {:?}", e);
        Err(e.into())
      }
    }
  } else {
    // For local storage fallback
    println!("Using local storage for highlight management");
    handle_local_undo_highlight(document_hash)
  }
}

/// Helper function to handle local undo highlight logic
fn handle_local_undo_highlight(document_hash: u64) -> Result<bool, Box<dyn std::error::Error>> {
  
  // For local storage fallback, check if there was a previous highlight action
  if let Some(_) = get_last_highlight_event(document_hash)? {
    // Record an undo event
    let event = Event::UndoHighlight {
      timestamp: Utc::now(),
      document_hash,
    };
    let serialized = serde_json::to_string(&event)?;
    let progress_file_path = get_progress_file_path()?;
    let mut file =
      OpenOptions::new().create(true).append(true).open(progress_file_path)?;
    file.write_all(serialized.as_bytes())?;
    file.write_all(b"\n")?;
    
    // Return true to indicate there was something to undo
    Ok(true)
  } else {
    // Return false to indicate there was nothing to undo
    Ok(false)
  }
}
