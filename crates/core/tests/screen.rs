use conn_core::screen::ScreenModel;

#[test]
fn ansi_colors_are_stripped() {
    let mut s = ScreenModel::new(5, 40);
    assert!(s.process(b"\x1b[31mred\x1b[0m text\r\n"));
    assert_eq!(s.rows()[0], "red text");
    assert_eq!(s.cursor().row, 1);
}

#[test]
fn cursor_movement_and_line_clear() {
    let mut s = ScreenModel::new(5, 40);
    s.process(b"hello world");
    s.process(b"\x1b[6G\x1b[K"); // col 6, clear to EOL
    assert_eq!(s.rows()[0], "hello");
    s.process(b"\x1b[2;1Hline2");
    assert_eq!(s.rows()[1], "line2");
}

#[test]
fn progress_rewrite_with_cr() {
    let mut s = ScreenModel::new(5, 40);
    s.process(b"10%\r20%\r100%");
    assert_eq!(s.rows()[0], "100%");
}

#[test]
fn revision_bumps_only_on_visible_change() {
    let mut s = ScreenModel::new(5, 40);
    assert_eq!(s.revision(), 0);
    assert!(s.process(b"a"));
    assert_eq!(s.revision(), 1);
    assert!(!s.process(b"\x1b[0m"));
    assert_eq!(s.revision(), 1);
}

#[test]
fn policy_vt_tracks_cursor_and_alt_screen() {
    let mut s = ScreenModel::new(10, 40);
    s.process(b"dev$ ls\r\nfoo\r\ndev$ ");
    assert_eq!(&s.rows()[..3], ["dev$ ls", "foo", "dev$"]);
    assert_eq!((s.cursor().row,s.cursor().col),(2,5));
    assert!(!s.alternate_screen());
    s.process(b"\x1b[?1049h");
    assert!(s.alternate_screen());
    s.process(b"\x1b[?1049l");
    assert!(!s.alternate_screen());
}

#[test]
fn resize_changes_size() {
    let mut s = ScreenModel::new(10, 40);
    s.resize(30, 100);
    let sz = s.size();
    assert_eq!((sz.rows, sz.cols), (30, 100));
}

#[test]
fn completed_lines_reflow_without_losing_unicode_or_conceal_attributes() {
    let mut s=ScreenModel::new(8,12);
    s.process("/long/가나다/끝\r\n\x1b[8mHIDDEN\x1b[0m visible\r\n$ ".as_bytes());
    let original=s.rows();
    s.resize(8,7);
    assert_eq!(&s.rows()[..3],["/long/","가나다/","끝"]);
    assert!(!s.rows().join("\n").contains("HIDDEN"));
    assert_eq!(s.cursor().row,5);
    s.resize(8,12);assert_eq!(s.rows(),original);
}

#[test]
fn height_resizes_keep_the_cursor_and_only_restore_visible_history() {
    let mut s=ScreenModel::new(4,20);s.process(b"one\r\ntwo\r\nthree\r\nfour\r\n$ ");
    assert_eq!(s.rows(),["two","three","four","$"]);
    s.resize(2,20);assert_eq!(s.rows(),["four","$"]);assert_eq!(s.cursor().row,1);
    s.resize(4,20);assert_eq!(s.rows(),["two","three","four","$"]);
    s.process(b"\x1b[3J");s.resize(6,20);
    assert_eq!(s.rows(),["two","three","four","$","",""]);
}

#[test]
fn alternate_screen_is_not_reflowed_and_normal_buffer_survives() {
    let mut s=ScreenModel::new(5,12);s.process(b"abcdefghijklmnop\r\n$ ");
    s.process(b"\x1b[?1049h\x1b[HALTERNATE123\r\n$ ");s.resize(5,8);
    assert_eq!(s.rows()[0],"ALTERNAT");s.process(b"\x1b[?1049l");
    assert_eq!(&s.rows()[..2],["abcdefgh","ijklmnop"]);
}

#[test]
fn concealed_split_sequences_wide_cells_and_equal_colors_are_not_observed() {
    let mut s=ScreenModel::new(5,80);
    for bytes in [b"ID: user Password: \x1b[".as_slice(),b"8mSECRET",b"\x1b[28m ******"] {s.process(bytes);}
    assert!(!s.rows().join("\n").contains("SECRET"));
    assert!(s.rows()[0].contains("******"));
    s.process("\r\n\x1b[8m암호\x1b[28mOK".as_bytes());
    assert_eq!(s.rows()[1],"    OK");
    s.process(b"\r\n\x1b[31;41mCOLOR_SECRET\x1b[0m visible");
    assert!(!s.rows().join("\n").contains("COLOR_SECRET"));
    s.process(b"\r\n\x1b[38;2;12;34;56;48;2;12;34;56mRGB_SECRET\x1b[0m");
    assert!(!s.rows().join("\n").contains("RGB_SECRET"));
    s.process(b"\r\n\x1b[8:0mSUBPARAM_SECRET\x1b[0m \x1b[1;8mCOMBINED_SECRET\x1b[0m shown");
    let text=s.rows().join("\n");
    assert!(!text.contains("SUBPARAM_SECRET")&&!text.contains("COMBINED_SECRET")&&text.contains("shown"));
}

#[test]
fn cursor_only_conceal_rewrites_and_alternate_screen_advance_revision() {
    let mut s=ScreenModel::new(3,30);s.process(b"visible");let r=s.revision();
    s.process(b"\r\x1b[8mvisible\x1b[0m");assert!(s.revision()>r);assert_eq!(s.rows()[0],"");
    let r=s.revision();s.process(b"\x1b[?25l");assert!(s.revision()>r);assert!(s.visible_cursor().is_none());
    s.process(b"\x1b[?1049hALT");assert!(s.alternate_screen());assert_eq!(s.rows()[0],"       ALT", "xterm alternate buffer inherits the current cursor");
    s.process(b"\x1b[?1049l");assert!(!s.alternate_screen());assert!(!s.rows().join("\n").contains("ALT"));
}
