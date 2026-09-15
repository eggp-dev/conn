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
fn projection_trims_trailing_empty_rows_and_tracks_alt_screen() {
    let mut s = ScreenModel::new(10, 40);
    s.process(b"dev$ ls\r\nfoo\r\ndev$ ");
    let p = s.projection();
    assert_eq!(p.screen, vec!["dev$ ls", "foo", "dev$"]);
    assert_eq!((p.cursor.row, p.cursor.col), (2, 5));
    assert!(!p.alternate_screen);
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
