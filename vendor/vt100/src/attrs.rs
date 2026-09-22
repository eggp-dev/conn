use crate::term::BufWrite as _;

/// Represents a foreground or background color for cells.
#[derive(Eq, PartialEq, Debug, Copy, Clone)]
pub enum Color {
    /// The default terminal color.
    Default,

    /// An indexed terminal color.
    Idx(u8),

    /// An RGB terminal color. The parameters are (red, green, blue).
    Rgb(u8, u8, u8),
}

impl Default for Color {
    fn default() -> Self {
        Self::Default
    }
}

const TEXT_MODE_BOLD: u8 = 0b0000_0001;
const TEXT_MODE_ITALIC: u8 = 0b0000_0010;
const TEXT_MODE_UNDERLINE: u8 = 0b0000_0100;
const TEXT_MODE_INVERSE: u8 = 0b0000_1000;

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Attrs {
    pub fgcolor: Color,
    pub bgcolor: Color,
    pub mode: u8,
}

impl Attrs {
    pub fn dim(&self) -> bool { self.mode & 0x20 != 0 }
    pub fn set_dim(&mut self, value: bool) { if value { self.mode |= 0x20; } else { self.mode &= !0x20; } }
    pub fn blink(&self) -> bool { self.mode & 0x40 != 0 }
    pub fn set_blink(&mut self, value: bool) { if value { self.mode |= 0x40; } else { self.mode &= !0x40; } }
    pub fn strike(&self) -> bool { self.mode & 0x80 != 0 }
    pub fn set_strike(&mut self, value: bool) { if value { self.mode |= 0x80; } else { self.mode &= !0x80; } }
    pub fn concealed(&self) -> bool { self.mode & 0x10 != 0 }
    pub fn set_concealed(&mut self, value: bool) { if value { self.mode |= 0x10; } else { self.mode &= !0x10; } }
    pub fn bold(&self) -> bool {
        self.mode & TEXT_MODE_BOLD != 0
    }

    pub fn set_bold(&mut self, bold: bool) {
        if bold {
            self.mode |= TEXT_MODE_BOLD;
        } else {
            self.mode &= !TEXT_MODE_BOLD;
        }
    }

    pub fn italic(&self) -> bool {
        self.mode & TEXT_MODE_ITALIC != 0
    }

    pub fn set_italic(&mut self, italic: bool) {
        if italic {
            self.mode |= TEXT_MODE_ITALIC;
        } else {
            self.mode &= !TEXT_MODE_ITALIC;
        }
    }

    pub fn underline(&self) -> bool {
        self.mode & TEXT_MODE_UNDERLINE != 0
    }

    pub fn set_underline(&mut self, underline: bool) {
        if underline {
            self.mode |= TEXT_MODE_UNDERLINE;
        } else {
            self.mode &= !TEXT_MODE_UNDERLINE;
        }
    }

    pub fn inverse(&self) -> bool {
        self.mode & TEXT_MODE_INVERSE != 0
    }

    pub fn set_inverse(&mut self, inverse: bool) {
        if inverse {
            self.mode |= TEXT_MODE_INVERSE;
        } else {
            self.mode &= !TEXT_MODE_INVERSE;
        }
    }

    pub fn write_escape_code_diff(
        &self,
        contents: &mut Vec<u8>,
        other: &Self,
    ) {
        if self != other && self == &Self::default() {
            crate::term::ClearAttrs::default().write_buf(contents);
            return;
        }

        let attrs = crate::term::Attrs::default();

        let attrs = if self.fgcolor == other.fgcolor {
            attrs
        } else {
            attrs.fgcolor(self.fgcolor)
        };
        let attrs = if self.bgcolor == other.bgcolor {
            attrs
        } else {
            attrs.bgcolor(self.bgcolor)
        };
        let attrs = if self.bold() == other.bold() {
            attrs
        } else {
            attrs.bold(self.bold())
        };
        let attrs = if self.italic() == other.italic() {
            attrs
        } else {
            attrs.italic(self.italic())
        };
        let attrs = if self.underline() == other.underline() {
            attrs
        } else {
            attrs.underline(self.underline())
        };
        let attrs = if self.inverse() == other.inverse() {
            attrs
        } else {
            attrs.inverse(self.inverse())
        };

        attrs.write_buf(contents);
        // 22 resets both bold and dim; reapply bold when only dim is removed.
        if self.dim() != other.dim() {
            contents.extend_from_slice(if self.dim() { b"\x1b[2m" } else { b"\x1b[22m" });
            if !self.dim() && self.bold() { contents.extend_from_slice(b"\x1b[1m"); }
        } else if self.dim() && self.bold() != other.bold() { contents.extend_from_slice(b"\x1b[2m"); }
        if self.blink() != other.blink() { contents.extend_from_slice(if self.blink() { b"\x1b[5m" } else { b"\x1b[25m" }); }
        if self.strike() != other.strike() { contents.extend_from_slice(if self.strike() { b"\x1b[9m" } else { b"\x1b[29m" }); }
        if self.concealed() != other.concealed() { contents.extend_from_slice(if self.concealed() { b"\x1b[8m" } else { b"\x1b[28m" }); }
    }
}
