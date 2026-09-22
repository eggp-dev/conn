/// A parser for terminal output which produces an in-memory representation of
/// the terminal contents.
pub struct Parser {
    parser: vte::Parser,
    screen: crate::screen::Screen,
    pending: Vec<u8>,
    pending_overflow: bool,
    last_print: bool,
}

impl Parser {
    /// Creates a new terminal parser of the given size and with the given
    /// amount of scrollback.
    #[must_use]
    pub fn new(rows: u16, cols: u16, scrollback_len: usize) -> Self {
        Self {
            parser: vte::Parser::new(),
            pending: Vec::new(),
            pending_overflow: false,
            last_print: false,
            screen: crate::screen::Screen::new(
                crate::grid::Size { rows, cols },
                scrollback_len,
            ),
        }
    }

    /// Processes the contents of the given byte string, and updates the
    /// in-memory terminal state.
    pub fn process(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            if !self.pending.is_empty() || byte == 0x1b || byte >= 0x80 {
                if self.pending.len() < 65_536 { self.pending.push(byte); }
                else { self.pending_overflow = true; }
            }
            let mut performer = CheckpointPerformer { screen: &mut self.screen, complete: false, executed: false, last_print: self.last_print };
            self.parser.advance(&mut performer, byte);
            self.last_print = performer.last_print;
            if performer.complete || matches!(byte, 0x18 | 0x1a) {
                self.pending.clear(); self.pending_overflow = false;
                // OSC/DCS may finish on ESC, which simultaneously begins ST.
                if byte == 0x1b { self.pending.push(byte); }
            } else if performer.executed && !self.pending.is_empty() {
                // C0 actions within a CSI have already affected the model.
                // Replaying them with the unfinished prefix would apply twice.
                self.pending.pop();
            }
        }
    }

    /// A complete owner checkpoint, including an unfinished output sequence.
    /// The tail is bounded, and oversized control strings fail explicitly.
    pub fn renderer_state_formatted(&self) -> Result<Vec<u8>, &'static str> {
        if self.pending_overflow { return Err("terminal checkpoint has an oversized incomplete control sequence"); }
        let mut out = self.screen.renderer_state_formatted();
        if self.last_print { self.screen.write_preceding_char(&mut out); }
        out.extend_from_slice(&self.pending);
        Ok(out)
    }

    /// Resizes the terminal.
    pub fn set_size(&mut self, rows: u16, cols: u16) {
        self.screen.set_size(rows, cols);
    }

    /// Scrolls to the given position in the scrollback.
    ///
    /// This position indicates the offset from the top of the screen, and
    /// should be `0` to put the normal screen in view.
    ///
    /// This affects the return values of methods called on `parser.screen()`:
    /// for instance, `parser.screen().cell(0, 0)` will return the top left
    /// corner of the screen after taking the scrollback offset into account.
    /// It does not affect `parser.process()` at all.
    ///
    /// The value given will be clamped to the actual size of the scrollback.
    pub fn set_scrollback(&mut self, rows: usize) {
        self.screen.set_scrollback(rows);
    }

    /// Resize normal-buffer soft wraps like the owner terminal. The alternate
    /// screen and the logical line containing the cursor are left for the
    /// running program to redraw. History remains bounded by `new`'s limit.
    pub fn set_size_reflow(&mut self, rows: u16, cols: u16) {
        self.screen.set_size_reflow(rows, cols);
    }

    /// Returns a reference to a `Screen` object containing the terminal
    /// state.
    #[must_use]
    pub fn screen(&self) -> &crate::screen::Screen {
        &self.screen
    }
}

impl Default for Parser {
    /// Returns a parser with dimensions 80x24 and no scrollback.
    fn default() -> Self {
        Self::new(24, 80, 0)
    }
}

impl std::io::Write for Parser {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.process(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

// Keep only an unfinished terminal-output sequence. Completed output is
// represented by the screen model, never a rolling byte-stream replay.
struct CheckpointPerformer<'a> { screen: &'a mut crate::screen::Screen, complete: bool, executed: bool, last_print: bool }
impl vte::Perform for CheckpointPerformer<'_> {
    fn print(&mut self, c: char) { self.screen.print(c); self.complete = true; self.last_print = true; }
    fn execute(&mut self, byte: u8) { self.screen.execute(byte); self.executed = true; self.last_print = false; }
    fn hook(&mut self, p: &vte::Params, i: &[u8], ignore: bool, action: char) { self.screen.hook(p, i, ignore, action); }
    fn put(&mut self, byte: u8) { self.screen.put(byte); }
    fn unhook(&mut self) { self.screen.unhook(); self.complete = true; self.last_print = false; }
    fn osc_dispatch(&mut self, p: &[&[u8]], bell: bool) { self.screen.osc_dispatch(p, bell); self.complete = true; self.last_print = false; }
    fn csi_dispatch(&mut self, p: &vte::Params, i: &[u8], ignore: bool, action: char) { self.screen.csi_dispatch(p, i, ignore, action); self.complete = true; self.last_print = false; }
    fn esc_dispatch(&mut self, i: &[u8], ignore: bool, byte: u8) { self.screen.esc_dispatch(i, ignore, byte); self.complete = true; self.last_print = false; }
}
