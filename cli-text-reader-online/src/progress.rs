use chrono::{DateTime, Utc};
use dirs::config_dir;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::fs::OpenOptions;
use std::hash::{Hash, Hasher};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

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
) -> Result<(), Box<dyn std::error::Error>> {
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
pub fn remove_highlight(
  document_hash: u64,
  line_number: usize,
) -> Result<(), Box<dyn std::error::Error>> {
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
pub fn clear_highlights(
  document_hash: u64,
) -> Result<(), Box<dyn std::error::Error>> {
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
pub fn load_highlights(
  document_hash: u64,
) -> Result<Vec<usize>, Box<dyn std::error::Error>> {
  let progress_file_path = get_progress_file_path()?;
  let file = OpenOptions::new().read(true).open(progress_file_path)?;
  let reader = io::BufReader::new(file);
  let mut highlights = Vec::new();
  let mut events_history: Vec<Event> = Vec::new();

  // First collect all highlight-related events for this document
  for line in reader.lines() {
    let line = line?;
    let event: Event = serde_json::from_str(&line)?;
    
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
  
  // Process events in chronological order
  for event in &events_history {
    match event {
      Event::AddHighlight { line_number, .. } => {
        if !highlights.contains(line_number) {
          highlights.push(*line_number);
        }
      },
      Event::RemoveHighlight { line_number, .. } => {
        highlights.retain(|&l| l != *line_number);
      },
      Event::ClearHighlights { .. } => {
        highlights.clear();
      },
      Event::UndoHighlight { .. } => {
        // We need to find the index of the current event we're processing
        // Find the last highlight event before this undo
        let current_idx = events_history.iter().position(|e| std::ptr::eq(e, event)).unwrap();
        
        if current_idx > 0 {
          // Get the most recent highlight action before this undo
          let prev_event = &events_history[current_idx-1];
          
          match prev_event {
            Event::AddHighlight { line_number, .. } => {
              // Undo an addition by removing the line
              highlights.retain(|&l| l != *line_number);
            },
            Event::RemoveHighlight { line_number, .. } => {
              // Undo a removal by adding the line back if it's not already there
              if !highlights.contains(line_number) {
                highlights.push(*line_number);
              }
            },
            Event::ClearHighlights { .. } => {
              // For a clear undo, we would ideally restore all previously highlighted lines
              // but since we don't store that information, we can't easily restore them
              // We'll leave it empty and the user will need to manually re-highlight
            },
            _ => {}
          }
        }
      },
      _ => {}
    }
  }

  Ok(highlights)
}

/// Export all highlights for a document as text
pub fn export_highlights(
  document_hash: u64,
  lines: &[String],
) -> Result<String, Box<dyn std::error::Error>> {
  let highlights = load_highlights(document_hash)?;
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
) -> Result<bool, Box<dyn std::error::Error>> {
  // First check if there was a previous highlight action
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
