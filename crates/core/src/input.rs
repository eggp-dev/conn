//! Agent-owned input only. Never reconstruct an uncertain command from one
//! terminal row: a wrapped row is not a shell command. Human input is tracked
//! separately without retaining its contents.

#[derive(Debug)]
pub struct InputTracker {
    buf: Vec<char>,
    pending: Vec<u8>,
    dirty: bool,
    cursor: usize,
    cursor_known: bool,
    prompt_col: Option<u16>,
}

impl Default for InputTracker {
    fn default() -> Self {
        Self {
            buf: Vec::new(),
            pending: Vec::new(),
            dirty: false,
            cursor: 0,
            cursor_known: true,
            prompt_col: None,
        }
    }
}
impl InputTracker {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn line(&self) -> String {
        self.buf.iter().collect()
    }
    pub fn has_pending(&self) -> bool {
        !self.buf.is_empty() || !self.pending.is_empty() || self.dirty
    }
    pub fn is_dirty(&self) -> bool {
        self.dirty || !self.pending.is_empty()
    }
    pub fn prompt_col(&self) -> Option<u16> {
        self.prompt_col
    }
    pub fn mark_uncertain(&mut self) {
        self.dirty = true;
    }
    pub fn reset(&mut self) {
        *self = Self::new();
    }
    fn edit(&mut self) {
        if !self.cursor_known {
            self.dirty = true;
        }
    }

    pub fn feed(&mut self, bytes: &[u8], cursor_col: u16) {
        let mut i = 0;
        while i < bytes.len() {
            self.prompt_col.get_or_insert(cursor_col);
            match bytes[i] {
                0x1b => {
                    // The named LEFT/RIGHT keys preserve the complete text.
                    // Non-ASCII readline movement varies with locale/graphemes;
                    // preserve the text, but require recovery before editing it.
                    if bytes[i..].starts_with(b"\x1b[D") || bytes[i..].starts_with(b"\x1b[C") {
                        self.cursor_known &= self.buf.iter().all(char::is_ascii);
                        if bytes[i + 2] == b'D' {
                            self.cursor = self.cursor.saturating_sub(1);
                        } else {
                            self.cursor = (self.cursor + 1).min(self.buf.len());
                        }
                        i += 3;
                        continue;
                    }
                    self.dirty = true;
                    i += 1;
                    if i < bytes.len() && matches!(bytes[i], b'[' | b'O') {
                        i += 1;
                        while i < bytes.len() && !(0x40..=0x7e).contains(&bytes[i]) {
                            i += 1;
                        }
                    }
                }
                b'\r' | b'\n' | 3 => self.reset(),
                1 => {
                    self.cursor = 0;
                    self.cursor_known = true;
                }
                5 => {
                    self.cursor = self.buf.len();
                    self.cursor_known = true;
                }
                21 => {
                    self.edit();
                    self.buf.drain(..self.cursor);
                    self.cursor = 0;
                }
                23 => {
                    self.edit();
                    while self.cursor > 0 && self.buf[self.cursor - 1].is_whitespace() {
                        self.cursor -= 1;
                        self.buf.remove(self.cursor);
                    }
                    while self.cursor > 0 && !self.buf[self.cursor - 1].is_whitespace() {
                        self.cursor -= 1;
                        self.buf.remove(self.cursor);
                    }
                }
                127 | 8 => {
                    // Readline locales and grapheme-aware bindings can erase a
                    // different number of Unicode scalars. Do not certify that
                    // resulting physical command using our scalar buffer.
                    if self.buf.iter().any(|c| !c.is_ascii()) { self.dirty = true; }
                    self.edit();
                    if self.cursor > 0 {
                        self.cursor -= 1;
                        self.buf.remove(self.cursor);
                    }
                }
                4 => {
                    if self.buf.iter().any(|c| !c.is_ascii()) { self.dirty = true; }
                    self.edit();
                    if self.cursor < self.buf.len() {
                        self.buf.remove(self.cursor);
                    }
                }
                0..=31 => self.dirty = true,
                b => {
                    self.edit();
                    self.pending.push(b);
                    self.flush_utf8();
                }
            }
            i += 1;
        }
    }

    fn flush_utf8(&mut self) {
        match std::str::from_utf8(&self.pending) {
            Ok(s) => {
                let chars: Vec<_> = s.chars().collect();
                let len = chars.len();
                self.buf.splice(self.cursor..self.cursor, chars);
                self.cursor += len;
                self.pending.clear();
            }
            Err(e) if e.error_len().is_some() => {
                self.dirty = true;
                self.pending.clear();
            }
            Err(_) => {}
        }
    }
}

/// Only a complete, reliable agent-owned command can enter the policy gate.
/// Completion, history and unknown editing never fall back to screen fragments.
pub fn resolve_command(tracked: &str, dirty: bool) -> Option<String> {
    (!dirty).then(|| tracked.trim().to_string())
}
