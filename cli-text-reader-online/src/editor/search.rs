use super::Editor;

pub fn find_next_match(editor: &mut Editor, forward: bool) {
    if editor.editor_state.search_query.is_empty() {
        return;
    }

    let start_line = if let Some((curr_line, _, _)) = editor.editor_state.current_match {
        curr_line
    } else {
        editor.offset
    };

    let direction = if forward { 1 } else { -1 };
    let total_lines = editor.lines.len();

    // Start from the current line or next/previous line based on direction
    let mut curr_line = if editor.editor_state.current_match.is_some() {
        if forward {
            (start_line + 1) % total_lines
        } else {
            if start_line == 0 {
                total_lines - 1
            } else {
                start_line - 1
            }
        }
    } else {
        start_line
    };

    let search_query = editor.editor_state.search_query.to_lowercase();
    let mut checked_lines = 0;

    // Search through all lines in the specified direction
    while checked_lines < total_lines {
        let line = &editor.lines[curr_line].to_lowercase();
        if let Some(start) = line.find(&search_query) {
            let end = start + search_query.len();
            editor.editor_state.current_match = Some((curr_line, start, end));
            break;
        }

        // Move to next/previous line based on direction
        if forward {
            curr_line = (curr_line + 1) % total_lines;
        } else {
            if curr_line == 0 {
                curr_line = total_lines - 1;
            } else {
                curr_line -= 1;
            }
        }

        checked_lines += 1;
    }

    if checked_lines >= total_lines {
        // No match found in the entire document
        editor.editor_state.current_match = None;
    }
}

pub fn center_on_match(editor: &mut Editor) {
    if let Some((line_idx, _, _)) = editor.editor_state.current_match {
        // Center the view on the matched line
        let center_offset = editor.height / 2;
        if line_idx >= center_offset {
            editor.offset = line_idx - center_offset;
        } else {
            editor.offset = 0;
        }
    }
}
