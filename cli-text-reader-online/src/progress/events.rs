use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Progress {
    pub document_hash: u64,
    pub offset: usize,
    pub total_lines: usize,
    pub percentage: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Event {
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
