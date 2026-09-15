//! Headless VT model. Used only for the agent projection; never shown to the human.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Size {
    pub rows: u16,
    pub cols: u16,
}

#[derive(Debug, Clone, Serialize)]
pub struct Cursor {
    pub row: u16,
    pub col: u16,
}

/// What the agent sees. Bounded by the visible screen, no scrollback.
#[derive(Debug, Clone, Serialize)]
pub struct Projection {
    pub revision: u64,
    pub size: Size,
    pub cursor: Cursor,
    pub screen: Vec<String>,
    #[serde(rename = "alternateScreen")]
    pub alternate_screen: bool,
}

pub struct ScreenModel {
    parser: vt100::Parser,
    revision: u64,
    last_contents: String,
}

impl ScreenModel {
    pub fn new(rows: u16, cols: u16) -> Self {
        let parser = vt100::Parser::new(rows.max(1), cols.max(1), 0);
        let last_contents = parser.screen().contents();
        Self { parser, revision: 0, last_contents }
    }

    /// Feed raw PTY output. Returns true if the visible screen changed.
    pub fn process(&mut self, bytes: &[u8]) -> bool {
        if bytes.is_empty() {
            return false;
        }
        self.parser.process(bytes);
        let contents = self.parser.screen().contents();
        if contents != self.last_contents {
            self.last_contents = contents;
            self.revision += 1;
            true
        } else {
            false
        }
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        self.parser.set_size(rows.max(1), cols.max(1));
        self.last_contents = self.parser.screen().contents();
        self.revision += 1;
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn size(&self) -> Size {
        let (rows, cols) = self.parser.screen().size();
        Size { rows, cols }
    }

    pub fn cursor(&self) -> Cursor {
        let (row, col) = self.parser.screen().cursor_position();
        Cursor { row, col }
    }

    pub fn alternate_screen(&self) -> bool {
        self.parser.screen().alternate_screen()
    }

    /// All rows, right-trimmed.
    pub fn rows(&self) -> Vec<String> {
        let (_, cols) = self.parser.screen().size();
        self.parser
            .screen()
            .rows(0, cols)
            .map(|r| r.trim_end().to_string())
            .collect()
    }

    /// The text of the row the cursor is on, right-trimmed.
    pub fn cursor_line(&self) -> String {
        let (row, _) = self.parser.screen().cursor_position();
        self.rows().into_iter().nth(row as usize).unwrap_or_default()
    }

    pub fn projection(&self) -> Projection {
        let mut rows = self.rows();
        while rows.last().map(|r| r.is_empty()).unwrap_or(false) {
            rows.pop();
        }
        Projection {
            revision: self.revision,
            size: self.size(),
            cursor: self.cursor(),
            screen: rows,
            alternate_screen: self.alternate_screen(),
        }
    }
}
