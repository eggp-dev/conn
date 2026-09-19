//! Shared terminal screen. PTY output is parsed once in the session core;
//! observation does not depend on an owner window or renderer heartbeat.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Size {
    pub rows: u16,
    pub cols: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cursor {
    pub row: u16,
    pub col: u16,
}

/// What the agent sees. Bounded by the visible screen, no scrollback.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Projection {
    #[serde(rename = "surfaceId")]
    pub surface_id: String,
    pub generation: u64,
    #[serde(rename = "outputSeq")]
    pub output_seq: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<SurfaceImage>,
    #[serde(rename = "imageUnavailable")]
    pub image_unavailable: bool,
    pub revision: u64,
    pub size: Size,
    pub cursor: Option<Cursor>,
    pub screen: Vec<String>,
    #[serde(rename = "alternateScreen")]
    pub alternate_screen: bool,
}

/// A revision of the session terminal grid.
/// No raw input, scrollback, or metadata outside the visible surface belongs here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SurfaceImage { pub mime_type: String, pub data: String }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SurfaceFrame {
    pub surface_id: String,
    pub generation: u64,
    pub revision: u64,
    pub output_seq: u64,
    pub rows: u16,
    pub cols: u16,
    pub cursor: Option<Cursor>,
    pub screen: Vec<String>,
    pub alternate_screen: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<SurfaceImage>,
    #[serde(default)]
    pub image_unavailable: bool,
}

impl SurfaceFrame {
    pub fn projection(&self) -> Projection {
        Projection { surface_id: self.surface_id.clone(), generation: self.generation, output_seq: self.output_seq,
            image: self.image.clone(), image_unavailable: self.image_unavailable, revision: self.revision, size: Size { rows: self.rows, cols: self.cols },
            cursor: self.cursor.clone(), screen: self.screen.clone(), alternate_screen: self.alternate_screen }
    }
}

pub struct ScreenModel {
    parser: vt100::Parser,
    revision: u64,
    last_contents: String,
}

impl ScreenModel {
    pub fn new(rows: u16, cols: u16) -> Self {
        let parser = vt100::Parser::new(rows.max(1), cols.max(1), 0);
        let last_contents = vec![""; usize::from(rows.max(1))].join("\n");
        Self { parser, revision: 0, last_contents }
    }

    /// Feed raw PTY output. Returns true if the visible screen changed.
    pub fn process(&mut self, bytes: &[u8]) -> bool {
        if bytes.is_empty() {
            return false;
        }
        let cursor = self.parser.screen().cursor_position();
        let hidden = self.parser.screen().hide_cursor();
        let alternate = self.parser.screen().alternate_screen();
        self.parser.process(bytes);
        let contents = self.rows().join("\n");
        if contents != self.last_contents || cursor != self.parser.screen().cursor_position()
            || hidden != self.parser.screen().hide_cursor() || alternate != self.parser.screen().alternate_screen() {
            self.last_contents = contents;
            self.revision += 1;
            true
        } else {
            false
        }
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        self.parser.set_size(rows.max(1), cols.max(1));
        self.last_contents = self.rows().join("\n");
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

    pub fn visible_cursor(&self) -> Option<Cursor> {
        (!self.parser.screen().hide_cursor()).then(|| self.cursor())
    }

    pub fn alternate_screen(&self) -> bool {
        self.parser.screen().alternate_screen()
    }

    /// Text the projection withholds: ANSI conceal, or explicit equal colours.
    fn hidden(cell: &vt100::Cell) -> bool {
        cell.concealed() || (cell.fgcolor() != vt100::Color::Default && cell.fgcolor() == cell.bgcolor())
    }

    /// The cursor row carries text that `rows()` withholds from observers.
    pub fn cursor_line_hidden(&self) -> bool {
        let screen = self.parser.screen();
        let (row, _) = screen.cursor_position();
        (0..screen.size().1).filter_map(|col| screen.cell(row, col)).any(|cell| cell.has_contents() && Self::hidden(cell))
    }

    /// All rows, right-trimmed.
    pub fn rows(&self) -> Vec<String> {
        let (_, cols) = self.parser.screen().size();
        let screen = self.parser.screen();
        (0..screen.size().0).map(|row| {
            let mut text = String::new();
            for col in 0..cols {
                let Some(cell) = screen.cell(row, col) else { continue; };
                if cell.is_wide_continuation() { continue; }
                if Self::hidden(cell) {
                    text.push_str(if cell.is_wide() { "  " } else { " " });
                } else if cell.has_contents() { text.push_str(&cell.contents()); }
                else { text.push(' '); }
            }
            text.trim_end().to_owned()
        }).collect()
    }

    /// The text of the row the cursor is on, right-trimmed. Policy input only:
    /// unlike `rows()` it keeps hidden text, so it must never reach an agent.
    pub fn cursor_line(&self) -> String {
        let (row, _) = self.parser.screen().cursor_position();
        self.parser.screen().rows(0, self.size().cols).nth(row as usize).unwrap_or_default().trim_end().to_owned()
    }


}
