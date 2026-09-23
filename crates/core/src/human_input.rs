//! Content-free bookkeeping for human input. Never retain the bytes, a command,
//! passwords, or an editor buffer. Empty-line recovery uses only an echoed return
//! to the previously visible empty input boundary.
use crate::screen::ScreenModel;

#[derive(Default)]
pub(crate) struct HumanInput {
    pub pending: bool,
    boundary: Option<(u16, u64)>,
    reconcile: bool,
    observed: bool,
}
impl HumanInput {
    pub fn remember_empty(&mut self, screen: &ScreenModel) {
        self.boundary = screen.input_boundary(); self.observed = false;
    }
    pub fn begin_cleanup(&mut self, screen: &ScreenModel) {
        self.pending = true;
        self.reconcile = true;
        self.observed |= self.boundary.is_some() && self.boundary != screen.input_boundary();
    }
    pub fn reset(&mut self) { *self = Self::default(); }

    pub fn note(&mut self, bytes: &[u8], screen: &ScreenModel, agent_pending: bool) {
        if bytes.is_empty() { return; }
        if matches!(bytes.last(), Some(b'\r' | b'\n' | 3)) {
            self.reset();
            return;
        }
        // Navigation/redraw/erase cannot add text to a known empty line. Do not
        // turn an empty Left or Ctrl-L into input_pending. Unknown sequences,
        // history, completion, paste and printable input remain conservative.
        let edit_only = only_empty_preserving_edits(bytes);
        if !self.pending && !agent_pending && edit_only { return; }
        if !self.pending {
            if !agent_pending { self.remember_empty(screen); }
        }
        self.observed |= self.boundary.is_some() && screen.input_boundary().is_some() && self.boundary != screen.input_boundary();
        self.pending = true;
        self.reconcile = edit_only;
    }

    pub fn watch_column(&self) -> Option<u16> {
        if self.pending && !self.observed { self.boundary.map(|b| b.0) } else { None }
    }

    pub fn output(&mut self, screen: &ScreenModel, moved: bool) {
        let boundary = screen.input_boundary();
        self.observed |= moved || (self.pending && self.boundary.is_some() && boundary.is_some() && self.boundary != boundary);
        if self.pending && self.reconcile && self.observed && self.boundary.is_some()
            && self.boundary == boundary {
            self.reset();
        }
    }
}

fn only_empty_preserving_edits(mut bytes: &[u8]) -> bool {
    while !bytes.is_empty() {
        // Readline cursor motion and deletion. Ctrl-U is an edit, NOT proof of
        // an empty line: with the cursor at Home it leaves the suffix intact.
        if matches!(bytes[0], 1 | 2 | 5 | 6 | 8 | 11 | 12 | 21 | 23 | 127) {
            bytes = &bytes[1..];
        } else if let Some(seq) = [b"\x1b[D".as_slice(), b"\x1b[C", b"\x1b[H", b"\x1b[F", b"\x1bOH", b"\x1bOF", b"\x1b[1~", b"\x1b[4~", b"\x1b[3~"].iter().find(|s| bytes.starts_with(s)) {
            bytes = &bytes[seq.len()..];
        } else { return false; }
    }
    true
}
