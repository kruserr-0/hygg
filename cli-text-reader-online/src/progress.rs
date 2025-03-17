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
pub fn add_highlight(
  document_hash: u64,
  line_number: usize,
  file_path: &str,
  client: Option<Arc<HyggClient>>,
) -> Result<(), Box<dyn std::error::Error>> {
  // If we have a client, use the server API
  if let Some(client) = client {
    // Create a runtime for async operations
    let rt = Runtime::new()?;
    rt.block_on(async {
      println!("Adding highlight for line {} via server API", line_number);
      client.add_highlight(file_path, &document_hash.to_string(), line_number).await?;
      Ok(())
    })
  } else {
    // Fallback to local storage for offline mode
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
}

/// Remove a highlight for a specific line in a document
pub fn remove_highlight(
  document_hash: u64,
  line_number: usize,
  file_path: &str,
  client: Option<Arc<HyggClient>>,
) -> Result<(), Box<dyn std::error::Error>> {
  // If we have a client, use the server API
  if let Some(client) = client {
    // Create a runtime for async operations
    let rt = Runtime::new()?;
    rt.block_on(async {
      println!("Removing highlight for line {} via server API", line_number);
      client.remove_highlight(file_path, &document_hash.to_string(), line_number).await?;
      Ok(())
    })
  } else {
    // Fallback to local storage for offline mode
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
}

/// Clear all highlights for a document
pub fn clear_highlights(
  document_hash: u64,
  client: Option<Arc<HyggClient>>,
) -> Result<(), Box<dyn std::error::Error>> {
  // If we have a client, use the server API
  if let Some(client) = client {
    // Create a runtime for async operations
    let rt = Runtime::new()?;
    rt.block_on(async {
      println!("Clearing all highlights via server API");
      client.clear_highlights(&document_hash.to_string()).await?;
      Ok(())
    })
  } else {
    // Fallback to local storage for offline mode
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
}

/// Load all highlighted lines for a document
pub fn load_highlights(
  document_hash: u64,
  client: Option<Arc<HyggClient>>,
) -> Result<Vec<usize>, Box<dyn std::error::Error>> {
  println!("Loading highlights for document hash: {}", document_hash);
  
  // If we have a client, use the server API
  if let Some(client) = client {
    // Create a runtime for async operations
    let rt = Runtime::new()?;
    let highlights = rt.block_on(async {
      println!("Loading highlights via server API");
      let server_highlights = client.get_highlights(&document_hash.to_string()).await?;
      
      // Extract just the line numbers
      let line_numbers: Vec<usize> = server_highlights.iter()
        .map(|h| h.line_number)
        .collect();
      
      println!("Received {} highlights from server", line_numbers.len());
      Ok::<Vec<usize>, Box<dyn std::error::Error>>(line_numbers)
    })?;
    
    return Ok(highlights);
  }
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
pub fn export_highlights(
  document_hash: u64,
  lines: &[String],
  client: Option<Arc<HyggClient>>,
) -> Result<String, Box<dyn std::error::Error>> {
  let highlights = load_highlights(document_hash, client)?;
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
pub fn undo_last_highlight(
  document_hash: u64,
  file_path: &str,
  client: Option<Arc<HyggClient>>,
) -> Result<bool, Box<dyn std::error::Error>> {
  // If we have a client, we'll need to use a different approach for server-side highlights
  if let Some(client) = client {
    // For server-side highlights, we use the client API
    // Create a runtime for async operations
    let rt = Runtime::new()?;
    let result = rt.block_on(async {
      println!("Undoing last highlight action via server API");
      // Call the server-side API to undo the last highlight action
      let was_undone = client.undo_last_highlight(&document_hash.to_string()).await?;
      println!("Server returned was_undone: {}", was_undone);
      Ok(was_undone)
    })?;
    
    return Ok(result);
  }
  
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
