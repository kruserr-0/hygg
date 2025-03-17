use chrono::Utc;
use dirs::config_dir;
use std::fs::OpenOptions;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use super::events::{Event, Progress};

fn get_progress_file_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut config_path = config_dir().ok_or("Unable to find config directory")?;
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

// Read events from progress file
pub fn read_events_from_file() -> Result<Vec<Event>, Box<dyn std::error::Error>> {
    let progress_file_path = get_progress_file_path()?;
    let file = OpenOptions::new().read(true).open(progress_file_path)?;
    let reader = io::BufReader::new(file);
    let mut events = Vec::new();

    for line in reader.lines() {
        let line = line?;
        if let Ok(event) = serde_json::from_str::<Event>(&line) {
            events.push(event);
        }
    }

    Ok(events)
}

// Write events to progress file (overwrite)
pub fn write_events_to_file(events: &[Event]) -> Result<(), Box<dyn std::error::Error>> {
    let progress_file_path = get_progress_file_path()?;
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(progress_file_path)?;

    for event in events {
        let serialized = serde_json::to_string(event)?;
        file.write_all(serialized.as_bytes())?;
        file.write_all(b"\n")?;
    }

    Ok(())
}
