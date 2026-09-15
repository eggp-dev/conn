//! Input line tracker. Reconstructs the command line from the bytes that were
//! written to the PTY, regardless of who wrote them.
//!
//! Tab completion and history navigation make the shell redraw the line, which the
//! tracker cannot follow. Those mark the tracker `dirty`; the session then falls back
//! to the VT model's cursor line at ENTER time.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineEvent {
    /// ENTER was pressed. Carries the tracked line as of that moment.
    Enter { tracked: String, dirty: bool, prompt_col: Option<u16> },
    /// Ctrl-C was pressed; the line was discarded.
    Interrupt,
}

#[derive(Debug, Default)]
pub struct InputTracker {
    buf: Vec<char>,
    pending: Vec<u8>,
    dirty: bool,
    prompt_col: Option<u16>,
}

impl InputTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn line(&self) -> String {
        self.buf.iter().collect()
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn prompt_col(&self) -> Option<u16> {
        self.prompt_col
    }

    pub fn reset(&mut self) {
        self.buf.clear();
        self.pending.clear();
        self.dirty = false;
        self.prompt_col = None;
    }

    fn note_first_byte(&mut self, cursor_col: u16) {
        if self.prompt_col.is_none() {
            self.prompt_col = Some(cursor_col);
        }
    }

    /// Feed bytes written to the PTY. `cursor_col` is the VT cursor column *before*
    /// these bytes were echoed; it is captured as the prompt width on the first byte
    /// of a new line.
    pub fn feed(&mut self, bytes: &[u8], cursor_col: u16) -> Vec<LineEvent> {
        let mut events = Vec::new();
        let mut i = 0;
        while i < bytes.len() {
            let b = bytes[i];
            match b {
                0x1b => {
                    // Escape sequence: consume it, mark dirty (cursor movement / history).
                    self.note_first_byte(cursor_col);
                    self.dirty = true;
                    i += 1;
                    if i < bytes.len() && (bytes[i] == b'[' || bytes[i] == b'O') {
                        i += 1;
                        while i < bytes.len() && !(0x40..=0x7e).contains(&bytes[i]) {
                            i += 1;
                        }
                    }
                    i += 1;
                    continue;
                }
                b'\r' | b'\n' => {
                    self.flush_pending();
                    if b == b'\n' && i > 0 && bytes[i - 1] == b'\r' {
                        i += 1;
                        continue;
                    }
                    events.push(LineEvent::Enter {
                        tracked: self.line(),
                        dirty: self.dirty,
                        prompt_col: self.prompt_col,
                    });
                    self.reset();
                }
                0x03 => {
                    events.push(LineEvent::Interrupt);
                    self.reset();
                }
                0x15 => {
                    // Ctrl-U: kill line
                    self.flush_pending();
                    self.buf.clear();
                    self.dirty = false;
                }
                0x17 => {
                    // Ctrl-W: kill word
                    self.flush_pending();
                    while self.buf.last().map(|c| c.is_whitespace()).unwrap_or(false) {
                        self.buf.pop();
                    }
                    while self.buf.last().map(|c| !c.is_whitespace()).unwrap_or(false) {
                        self.buf.pop();
                    }
                }
                0x7f | 0x08 => {
                    self.flush_pending();
                    self.buf.pop();
                }
                b'\t' => {
                    self.note_first_byte(cursor_col);
                    self.dirty = true;
                }
                0x00..=0x1f => {
                    // Other control chars (Ctrl-A/E cursor moves etc.): tracking is unreliable.
                    self.note_first_byte(cursor_col);
                    if b != 0x04 {
                        self.dirty = true;
                    }
                }
                _ => {
                    self.note_first_byte(cursor_col);
                    self.pending.push(b);
                    self.flush_complete_utf8();
                }
            }
            i += 1;
        }
        events
    }

    fn flush_complete_utf8(&mut self) {
        loop {
            match std::str::from_utf8(&self.pending) {
                Ok(s) => {
                    self.buf.extend(s.chars());
                    self.pending.clear();
                    return;
                }
                Err(e) => {
                    let valid = e.valid_up_to();
                    if valid > 0 {
                        let s = std::str::from_utf8(&self.pending[..valid]).unwrap();
                        self.buf.extend(s.chars());
                        self.pending.drain(..valid);
                        continue;
                    }
                    if e.error_len().is_some() {
                        // invalid byte: drop it
                        self.pending.remove(0);
                        continue;
                    }
                    return; // incomplete sequence, wait for more
                }
            }
        }
    }

    fn flush_pending(&mut self) {
        if !self.pending.is_empty() {
            let s = String::from_utf8_lossy(&self.pending).to_string();
            self.buf.extend(s.chars());
            self.pending.clear();
        }
    }
}

/// Decide the command text at ENTER time.
///
/// * Not dirty → the tracked line.
/// * Dirty → the VT cursor line with the prompt prefix removed. If the two differ,
///   the VT model wins. Falls back to the tracked line when the VT line yields nothing.
pub fn resolve_command(tracked: &str, dirty: bool, cursor_line: &str, prompt_col: Option<u16>) -> String {
    let from_vt = strip_prompt(cursor_line, prompt_col, tracked);
    if !dirty {
        return tracked.trim().to_string();
    }
    match from_vt {
        Some(v) if !v.trim().is_empty() => v.trim().to_string(),
        _ => tracked.trim().to_string(),
    }
}

/// Like `resolve_command` but always prefers the VT line when it is available and
/// differs from the tracked line. This is a display helper, not a safe policy or
/// audit source: the cursor row may contain only the end of a wrapped command.
pub fn resolve_command_prefer_vt(tracked: &str, cursor_line: &str, prompt_col: Option<u16>) -> String {
    match strip_prompt(cursor_line, prompt_col, tracked) {
        Some(v) if !v.trim().is_empty() => v.trim().to_string(),
        _ => tracked.trim().to_string(),
    }
}

fn strip_prompt(cursor_line: &str, prompt_col: Option<u16>, tracked: &str) -> Option<String> {
    let chars: Vec<char> = cursor_line.chars().collect();
    if let Some(col) = prompt_col {
        let col = col as usize;
        if col <= chars.len() {
            return Some(chars[col..].iter().collect());
        }
    }
    // No prompt width known: if the tracked text appears at the end of the line, take it.
    let t = tracked.trim();
    if !t.is_empty() && cursor_line.trim_end().ends_with(t) {
        return Some(t.to_string());
    }
    None
}
