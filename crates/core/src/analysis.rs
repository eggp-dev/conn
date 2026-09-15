//! Shell-structure analysis of a command line. Deterministic and deliberately
//! conservative: it splits a line into segments, normalises what each segment
//! actually runs, flags constructs that hide their real command, follows `cd`
//! to resolve targets against the shell's cwd, and hands the pieces to policy.
//!
//! This catches the mistakes and the casual evasions (`\rm`, `/bin/rm`,
//! `command rm`, `r''m`, `sudo rm`, `cd .. && rm`, `sh -c '…'`). It does not
//! claim to catch a determined adversary — see docs/security.md.

use std::path::{Path, PathBuf};

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Segment {
    /// The segment text as typed (trimmed).
    pub text: String,
    /// Tokenised words, quotes removed.
    pub argv: Vec<String>,
    /// The command that will actually run, after peeling wrappers and prefixes.
    pub command: Option<String>,
    /// Arguments after the real command.
    pub args: Vec<String>,
    /// Why this segment cannot be inspected further, if so (`sh -c`, `eval`, `$(...)`, …).
    pub opaque: Option<String>,
    /// Wrappers peeled off in front of the real command (`sudo`, `env`, `nohup`, …).
    pub wrappers: Vec<String>,
}

/// Tokenise like a POSIX shell for the purpose of inspection: quotes group,
/// backslash escapes, `$(`…`)` and backticks are recorded as substitutions.
fn tokenise(text: &str) -> (Vec<String>, bool) {
    let mut words = Vec::new();
    let mut cur = String::new();
    let mut in_word = false;
    let mut has_subst = false;
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '\'' => {
                in_word = true;
                i += 1;
                while i < chars.len() && chars[i] != '\'' {
                    cur.push(chars[i]);
                    i += 1;
                }
            }
            '"' => {
                in_word = true;
                i += 1;
                while i < chars.len() && chars[i] != '"' {
                    if chars[i] == '\\' && i + 1 < chars.len() {
                        i += 1;
                    }
                    // `$(cmd)` is a substitution; `$((expr))` is arithmetic and is not.
                    if chars[i] == '$' && i + 1 < chars.len() && chars[i + 1] == '(' && !(i + 2 < chars.len() && chars[i + 2] == '(') {
                        has_subst = true;
                    }
                    if chars[i] == '`' {
                        has_subst = true;
                    }
                    cur.push(chars[i]);
                    i += 1;
                }
            }
            '\\' => {
                in_word = true;
                if i + 1 < chars.len() {
                    cur.push(chars[i + 1]);
                    i += 1;
                }
            }
            '`' => {
                has_subst = true;
                in_word = true;
                cur.push(c);
            }
            '$' if i + 1 < chars.len() && chars[i + 1] == '(' => {
                // `$((…))` is arithmetic expansion, not a command substitution.
                if !(i + 2 < chars.len() && chars[i + 2] == '(') {
                    has_subst = true;
                }
                in_word = true;
                cur.push(c);
            }
            c if c.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut cur));
                    in_word = false;
                }
            }
            _ => {
                in_word = true;
                cur.push(c);
            }
        }
        i += 1;
    }
    if in_word {
        words.push(cur);
    }
    (words, has_subst)
}

/// Split on `&&`, `||`, `;`, `|`, `&` and newlines outside quotes. Parenthesised
/// groups are unwrapped and split recursively.
pub fn split_line(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    let mut depth_paren = 0i32;
    let mut depth_brace = 0i32;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '\'' => {
                cur.push(c);
                i += 1;
                while i < chars.len() && chars[i] != '\'' {
                    cur.push(chars[i]);
                    i += 1;
                }
                if i < chars.len() {
                    cur.push('\'');
                }
            }
            '"' => {
                cur.push(c);
                i += 1;
                while i < chars.len() && chars[i] != '"' {
                    if chars[i] == '\\' && i + 1 < chars.len() {
                        cur.push(chars[i]);
                        i += 1;
                    }
                    cur.push(chars[i]);
                    i += 1;
                }
                if i < chars.len() {
                    cur.push('"');
                }
            }
            '\\' => {
                cur.push(c);
                if i + 1 < chars.len() {
                    cur.push(chars[i + 1]);
                    i += 1;
                }
            }
            '(' => {
                depth_paren += 1;
                cur.push(c);
            }
            ')' => {
                depth_paren -= 1;
                cur.push(c);
            }
            '{' => {
                depth_brace += 1;
                cur.push(c);
            }
            '}' => {
                depth_brace -= 1;
                cur.push(c);
            }
            '&' | '|' | ';' | '\n' if depth_paren <= 0 && depth_brace <= 0 => {
                // `&&` / `||` / `|&` / `;;`
                if i + 1 < chars.len() && (chars[i + 1] == c || (c == '|' && chars[i + 1] == '&')) {
                    i += 1;
                }
                let s = cur.trim().to_string();
                if !s.is_empty() {
                    out.push(s);
                }
                cur.clear();
            }
            _ => cur.push(c),
        }
        i += 1;
    }
    let s = cur.trim().to_string();
    if !s.is_empty() {
        out.push(s);
    }
    // unwrap subshells / groups and recurse
    let mut flat = Vec::new();
    for s in out {
        let t = s.trim();
        let inner = if t.starts_with('(') && t.ends_with(')') {
            Some(&t[1..t.len() - 1])
        } else if t.starts_with('{') && t.ends_with('}') {
            Some(&t[1..t.len() - 1])
        } else {
            None
        };
        match inner {
            Some(inner) if inner.trim() != t => flat.extend(split_line(inner)),
            _ => flat.push(s),
        }
    }
    flat
}

const PREFIX_WRAPPERS: &[&str] = &["command", "builtin", "exec", "nohup", "time", "nice", "ionice", "caffeinate", "doas", "sudo", "env", "stdbuf", "unbuffer"];

fn basename(word: &str) -> String {
    let w = word.trim_start_matches('\\');
    Path::new(w).file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_else(|| w.to_string())
}

/// Peel `VAR=x`, `sudo`, `env`, `command`, `\`, `/usr/bin/` … to find the real command.
fn resolve_command(argv: &[String]) -> (Option<String>, Vec<String>, Vec<String>) {
    let mut i = 0;
    let mut wrappers = Vec::new();
    loop {
        if i >= argv.len() {
            return (None, vec![], wrappers);
        }
        let w = &argv[i];
        // leading assignments
        if w.contains('=') && !w.starts_with('-') && w.split('=').next().map(|k| !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')).unwrap_or(false) {
            i += 1;
            continue;
        }
        let base = basename(w);
        if PREFIX_WRAPPERS.contains(&base.as_str()) {
            wrappers.push(base.clone());
            i += 1;
            // skip the wrapper's own flags (sudo -u root, env -i, nice -n 5 …)
            while i < argv.len() && argv[i].starts_with('-') {
                let takes_value = matches!((base.as_str(), argv[i].as_str()), ("sudo", "-u") | ("sudo", "-g") | ("nice", "-n") | ("doas", "-u") | ("ionice", "-c") | ("ionice", "-n"));
                i += if takes_value { 2 } else { 1 };
            }
            continue;
        }
        return (Some(base), argv[i + 1..].to_vec(), wrappers);
    }
}

fn opaque_reason(command: &str, args: &[String], has_subst: bool) -> Option<String> {
    if has_subst {
        return Some("command substitution ($( ) / ` `)".into());
    }
    let has = |f: &str| args.iter().any(|a| a == f);
    match command {
        "sh" | "bash" | "zsh" | "dash" | "ksh" | "fish" => Some("shell -c / script".into()).filter(|_| has("-c") || args.iter().any(|a| !a.starts_with('-'))),
        "eval" | "source" | "." => Some("eval / source".into()),
        "xargs" => Some("xargs".into()),
        "find" if has("-delete") || has("-exec") || has("-execdir") || has("-ok") => Some("find -delete / -exec".into()),
        "python" | "python3" | "perl" | "ruby" | "node" | "php" | "lua" if has("-c") || has("-e") => Some("inline script (-c / -e)".into()),
        "ssh" | "mosh" => Some("remote execution (ssh)".into()),
        "docker" if args.first().map(|a| a == "exec" || a == "run").unwrap_or(false) => Some("container exec".into()),
        "kubectl" if args.first().map(|a| a == "exec").unwrap_or(false) => Some("container exec".into()),
        "su" | "script" | "screen" | "tmux" if command != "tmux" || args.first().map(|a| a == "send-keys").unwrap_or(false) => Some("runs in another session".into()),
        _ => None,
    }
}

pub fn analyse_segment(text: &str) -> Segment {
    let (argv, has_subst) = tokenise(text);
    let (command, args, wrappers) = resolve_command(&argv);
    let opaque = match &command {
        Some(c) => opaque_reason(c, &args, has_subst),
        None if has_subst => Some("command substitution ($( ) / ` `)".into()),
        None => None,
    };
    Segment { text: text.trim().to_string(), argv, command, args, opaque, wrappers }
}

pub fn analyse_line(line: &str) -> Vec<Segment> {
    split_line(line).into_iter().map(|s| analyse_segment(&s)).collect()
}

/// Commands whose non-flag arguments are filesystem targets worth resolving.
pub const PATH_COMMANDS: &[&str] = &["rm", "rmdir", "mv", "cp", "chmod", "chown", "chgrp", "truncate", "shred", "dd", "ln", "unlink", "rsync", "git"];

/// Expand `~`, join with `cwd`, and normalise `.`/`..` without touching the filesystem.
pub fn resolve_path(cwd: &Path, home: Option<&Path>, arg: &str) -> PathBuf {
    let expanded: PathBuf = if arg == "~" {
        home.map(|h| h.to_path_buf()).unwrap_or_else(|| PathBuf::from(arg))
    } else if let Some(rest) = arg.strip_prefix("~/") {
        home.map(|h| h.join(rest)).unwrap_or_else(|| PathBuf::from(arg))
    } else {
        PathBuf::from(arg)
    };
    let joined = if expanded.is_absolute() { expanded } else { cwd.join(expanded) };
    let mut out = PathBuf::new();
    for comp in joined.components() {
        use std::path::Component::*;
        match comp {
            CurDir => {}
            ParentDir => {
                out.pop();
            }
            c => out.push(c),
        }
    }
    out
}

/// Walk segments in order, applying `cd`, and return the cwd in effect *before*
/// segment `idx` runs.
pub fn cwd_before(segments: &[Segment], idx: usize, start: &Path, home: Option<&Path>) -> PathBuf {
    let mut cwd = start.to_path_buf();
    for s in &segments[..idx] {
        if s.command.as_deref() == Some("cd") {
            match s.args.iter().find(|a| !a.starts_with('-')) {
                None => {
                    if let Some(h) = home {
                        cwd = h.to_path_buf();
                    }
                }
                Some(a) if a == "-" => {}
                Some(a) => cwd = resolve_path(&cwd, home, a),
            }
        } else if s.command.as_deref() == Some("pushd") {
            if let Some(a) = s.args.iter().find(|a| !a.starts_with('-')) {
                cwd = resolve_path(&cwd, home, a);
            }
        }
    }
    cwd
}

/// Filesystem targets of a segment (non-flag args), resolved against `cwd`.
pub fn targets(segment: &Segment, cwd: &Path, home: Option<&Path>) -> Vec<PathBuf> {
    let Some(cmd) = &segment.command else { return vec![] };
    if !PATH_COMMANDS.contains(&cmd.as_str()) {
        return vec![];
    }
    let mut args = segment.args.iter();
    let mut out = Vec::new();
    if cmd == "git" {
        // only destructive subcommands touch paths in a way worth showing
        let sub = args.next().map(|s| s.as_str()).unwrap_or("");
        if !matches!(sub, "clean" | "checkout" | "restore" | "reset" | "rm") {
            return vec![];
        }
    }
    let mut after_dashdash = false;
    for a in args {
        if after_dashdash || !a.starts_with('-') {
            out.push(resolve_path(cwd, home, a));
        } else if a == "--" {
            after_dashdash = true;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_on_operators_outside_quotes() {
        let s = split_line("cd .. && rm -rf -- hello-mel; echo 'a && b' | wc -l");
        assert_eq!(s, vec!["cd ..", "rm -rf -- hello-mel", "echo 'a && b'", "wc -l"]);
    }

    #[test]
    fn unwraps_subshells() {
        assert_eq!(split_line("(cd /tmp && rm -rf x) && ls"), vec!["cd /tmp", "rm -rf x", "ls"]);
    }

    #[test]
    fn normalises_the_real_command() {
        for line in ["\\rm -rf x", "/bin/rm -rf x", "command rm -rf x", "r''m -rf x", "\"rm\" -rf x", "sudo -u root rm -rf x", "env FOO=1 rm -rf x", "FOO=1 nohup rm -rf x"] {
            let seg = analyse_segment(line);
            assert_eq!(seg.command.as_deref(), Some("rm"), "{line}");
            assert_eq!(seg.args, vec!["-rf", "x"], "{line}");
        }
    }

    #[test]
    fn flags_opaque_constructs() {
        assert!(analyse_segment("sh -c 'rm -rf x'").opaque.is_some());
        assert!(analyse_segment("bash script.sh").opaque.is_some());
        assert!(analyse_segment("eval \"$X\"").opaque.is_some());
        assert!(analyse_segment("find . -name '*.log' -delete").opaque.is_some());
        assert!(analyse_segment("python3 -c 'import shutil'").opaque.is_some());
        assert!(analyse_segment("echo $(rm -rf x)").opaque.is_some());
        assert!(analyse_segment("cat `ls`").opaque.is_some());
        assert!(analyse_segment("echo hello-$((6*7))").opaque.is_none());
        assert!(analyse_segment("echo \"n=$((1+2))\"").opaque.is_none());
        assert!(analyse_segment("ls -la").opaque.is_none());
        assert!(analyse_segment("find . -name '*.log'").opaque.is_none());
        assert!(analyse_segment("python3 script.py").opaque.is_none());
    }

    #[test]
    fn follows_cd_and_resolves_targets() {
        let segs = analyse_line("cd .. && rm -rf -- hello-mel && echo done");
        let cwd = cwd_before(&segs, 1, Path::new("/Users/e/dev/hello-mel"), Some(Path::new("/Users/e")));
        assert_eq!(cwd, PathBuf::from("/Users/e/dev"));
        let t = targets(&segs[1], &cwd, Some(Path::new("/Users/e")));
        assert_eq!(t, vec![PathBuf::from("/Users/e/dev/hello-mel")]);
        let segs = analyse_line("cd ~/x; rm -r ./y ../z");
        let cwd = cwd_before(&segs, 1, Path::new("/tmp"), Some(Path::new("/Users/e")));
        assert_eq!(targets(&segs[1], &cwd, Some(Path::new("/Users/e"))), vec![PathBuf::from("/Users/e/x/y"), PathBuf::from("/Users/e/z")]);
    }
}
