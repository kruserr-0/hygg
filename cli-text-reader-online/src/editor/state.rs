#[derive(PartialEq)]
pub enum EditorMode {
    Normal,
    Command,
    Search,
    ReverseSearch,
    Visual,
}

pub struct EditorState {
    pub mode: EditorMode,
    pub command_buffer: String,
    pub search_query: String,
    pub search_direction: bool, // true for forward, false for backward
    pub last_search_index: Option<usize>,
    pub current_match: Option<(usize, usize, usize)>, // (line_index, start, end)
    pub visual_start: Option<usize>, // Starting line for visual mode selection
    pub visual_end: Option<usize>,   // Ending line for visual mode selection
}

impl EditorState {
    pub fn new() -> Self {
        Self {
            mode: EditorMode::Normal,
            command_buffer: String::new(),
            search_query: String::new(),
            search_direction: true,
            last_search_index: None,
            current_match: None,
            visual_start: None,
            visual_end: None,
        }
    }
}
