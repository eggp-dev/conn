//! Presented surface contract and internal command-policy VT tracker.
//! Only a native-owned presented surface may become an agent observation.

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

/// A completed frame from the actual owner-rendered terminal viewport.
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
    pub visible: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<SurfaceImage>,
    #[serde(default)]
    pub image_unavailable: bool,
}

impl SurfaceFrame {
    pub fn validate(&self) -> bool {
        !self.surface_id.is_empty() && self.surface_id.len() <= 128
            && self.rows > 0 && self.rows <= 512 && self.cols > 0 && self.cols <= 1024
            && self.screen.len() <= usize::from(self.rows)
            && self.screen.iter().map(String::len).sum::<usize>() <= 1024 * 1024
            && self.screen.iter().all(|s| !s.chars().any(char::is_control))
            && self.image.as_ref().is_none_or(|i| i.mime_type == "image/png" && i.data.len() <= 4 * 1024 * 1024)
            && self.cursor.as_ref().is_none_or(|c| c.row < self.rows && c.col < self.cols)
    }

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


}
