mod common;
use conn_core::{screen::Size, session::Session};

fn checkpoint(s: &Session) -> vt100::Parser {
    let c = s.renderer_checkpoint().unwrap();
    let Size { rows, cols } = c.size;
    let mut restored = vt100::Parser::new(rows, cols, 5000);
    restored.process(&c.data);
    restored
}
fn same(a: &vt100::Screen, b: &vt100::Screen) {
    assert_eq!(a.contents(), b.contents(), "visible contents");
    assert_eq!(a.cursor_position(), b.cursor_position(), "cursor");
    assert_eq!(a.alternate_screen(), b.alternate_screen(), "alternate screen");
    assert_eq!(a.hide_cursor(), b.hide_cursor());
    assert_eq!(a.application_keypad(), b.application_keypad());
    assert_eq!(a.application_cursor(), b.application_cursor());
    assert_eq!(a.bracketed_paste(), b.bracketed_paste());
    assert_eq!(a.mouse_protocol_mode(), b.mouse_protocol_mode());
    assert_eq!(a.mouse_protocol_encoding(), b.mouse_protocol_encoding());
    for row in 0..a.size().0 {
        assert_eq!(a.row_wrapped(row), b.row_wrapped(row), "soft wrap row {row}");
        for col in 0..a.size().1 { assert_eq!(a.cell(row,col), b.cell(row,col), "cell {row}:{col}"); }
    }
}
#[test]
fn checkpoint_preserves_output_state_and_following_bytes_at_every_split() {
    let fixtures: Vec<Vec<u8>> = vec![
        format!("{}tail", "first-long-line-123456789012345678901234567890\r\n".repeat(12)).into_bytes(),
        b"\x1b[1;2;5;9;31mstyled\x1b[22mnormal-dim\x1b[0m \x1b[2mDIM\x1b[1mBOTH\x1b[22mRESET".to_vec(),
        b"\x1b[31;44;1mcolor\x1b[0m\r\n12345678901234567890NEXT\x1b[3;8H\x1b7\x1b[4;2Hhello\x1b8!".to_vec(),
        "한글 🚀 combined e\u{301}\r\n\x1b[38;2;20;30;40m색상\x1b[0m".as_bytes().to_vec(),
        b"NORMAL\x1b[2;4H\x1b[32m\x1b[?1049h\x1b[?25lALT\x1b[2;5r\x1b[?6h\x1b[2;3Hmore\nrows\x1b[?1049l!".to_vec(),
        b"\x1b[?2004h\x1b[?1h\x1b=\x1b[?1002h\x1b[?1006h\x1b[?1004h\x1b[?7l123456789012345678901234\x1b[2;1H\x1b[4habc\x1b[1Ginsert\x1b[4l".to_vec(),
        b"\x1b]2;checkpoint title\x1b\\before\x1b[31mred\x1b[4;\x08\x07;1Hafter".to_vec(),
    ];
    let mut exported = Vec::new();
    for (fixture, bytes) in fixtures.iter().enumerate() {
        for split in 0..=bytes.len() {
            let mut h = common::Harness::new();
            h.session.resize(6, 20).unwrap();
            h.session.pty_output(&bytes[..split]);
            let mut original = vt100::Parser::new(6,20,5000);
            original.process(&bytes[..split]);
            let mut restored = checkpoint(&h.session);
            exported.push(serde_json::json!({"fixture":fixture,"split":split,"rows":6,"cols":20,"before":bytes[..split],"checkpoint":h.session.renderer_checkpoint().unwrap().data,"after":bytes[split..]}));
            same(original.screen(), restored.screen());
            original.process(&bytes[split..]); restored.process(&bytes[split..]);
            same(original.screen(), restored.screen());
            // Restored saved cursor/margins/attributes must affect subsequent IO.
            original.process(b"\x1b8Z\nlast"); restored.process(b"\x1b8Z\nlast");
            same(original.screen(), restored.screen());
            assert!(h.session.renderer_checkpoint().is_ok(), "fixture {fixture} split {split}");
        }
    }
    if let Ok(path) = std::env::var("CONN_CHECKPOINT_FIXTURES") { std::fs::write(path, serde_json::to_vec(&exported).unwrap()).unwrap(); }
}
#[test]
fn checkpoint_is_owner_only_does_not_expose_hidden_input_or_change_sequence() {
    let mut h = common::Harness::new();
    h.session.pty_output(b"shown\x1b[8mconcealed\x1b[28m");
    let before=h.session.output_seq();
    h.session.human_input(b"not-echoed-password").unwrap();
    let c=h.session.renderer_checkpoint().unwrap();
    assert_eq!(c.output_seq,before);
    assert!(!String::from_utf8_lossy(&c.data).contains("not-echoed-password"));
    assert!(!h.session.authoritative_surface().unwrap().screen.join("\n").contains("concealed"));
}


#[test]
fn one_row_history_and_oversized_incomplete_control_state() {
    let mut h = common::Harness::new();
    h.session.resize(1, 8).unwrap();
    let bytes = b"abcdefghijk\r\nABCDEFGHIJK\r\nlast";
    h.session.pty_output(bytes);
    let mut original=vt100::Parser::new(1,8,5000); original.process(bytes);
    let restored=checkpoint(&h.session);
    same(original.screen(),restored.screen());
    h.session.pty_output(b"\x1b]");
    h.session.pty_output(&vec![b'x'; 65_537]);
    assert!(h.session.renderer_checkpoint().is_err(), "never restore a truncated unfinished sequence");
    h.session.pty_output(b"\x07");
    assert!(h.session.renderer_checkpoint().is_ok());
}
