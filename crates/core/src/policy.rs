//! User-declared policy: `deny` → `confirm` → `default`, first match wins.
//!
//! Patterns are regular expressions matched against the command line as typed.
//! This is a mistake-prevention device, not a security boundary (see docs/security.md).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use regex::Regex;
use serde::{Deserialize, Serialize};

pub const EXAMPLE_POLICY: &str = r#"# conn policy
#
# Evaluation order: deny → confirm → default. The first match wins.
# A deny cannot be lifted by approval. Patterns are regular expressions.

# Rule kinds
#   pattern:   regex, matched against the text of one segment (a line is split on &&, ;, |)
#   command:   the command that actually runs — \rm, /bin/rm, command rm, sudo rm and r''m all count as rm
#              args: optional regex over the joined arguments
#   protected: path globs — a deny when rm/mv/chmod etc. would touch one of them

deny:
  - pattern: 'rm -rf /(\s|$)'
    label: 'delete root'
  - pattern: ':\(\)\{.*\};:'
    label: 'fork bomb'

confirm:
  - command: rm
    args: '(^|\s)-[a-zA-Z]*[rR]'
    label: 'recursive delete'
  - command: rm
    label: 'delete files'
  - command: git
    args: '^(clean|reset --hard|push .*--force|push .*-f\b)'
    label: 'destructive git'
  - pattern: 'kubectl delete'
    label: 'delete resource'
  - command: sudo
    label: 'privilege escalation'
  - pattern: '(^|[^>&])>[^>&]'
    label: 'overwrite file'
  - pattern: 'DROP TABLE'
    label: 'drop table'

# Deny when a delete/move/chmod target matches one of these globs (~ allowed)
protected:
  - '~/.ssh/**'
  - '~/.conn/**'

# Constructs whose effect cannot be inspected (sh -c, eval, xargs, find -delete, python -c, ssh …)
opaque: confirm

# When a confirm/deny-level segment shares a line with other segments, deny instead of
# asking and send it back with "run it alone". One approval = one dangerous action.
isolate_dangerous: true

# An agent's ENTER must carry a one-line intent
require_intent: true

default: allow
"#;

#[derive(Debug, Clone, Deserialize)]
struct RuleFile {
    #[serde(default)]
    pattern: Option<String>,
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    args: Option<String>,
    label: String,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
struct PolicyFile {
    #[serde(default)]
    deny: Vec<RuleFile>,
    #[serde(default)]
    confirm: Vec<RuleFile>,
    #[serde(default)]
    protected: Vec<String>,
    #[serde(default = "default_opaque")]
    opaque: DefaultDecision,
    #[serde(default = "default_true")]
    isolate_dangerous: bool,
    #[serde(default = "default_true")]
    require_intent: bool,
    #[serde(default)]
    default: DefaultDecision,
}


fn default_opaque() -> DefaultDecision {
    DefaultDecision::Confirm
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DefaultDecision {
    #[default]
    Allow,
    Confirm,
    Deny,
}

#[derive(Debug, Clone)]
pub enum Matcher {
    /// Regex over the segment text.
    Pattern(Regex),
    /// Normalised command name, optionally with a regex over the joined args.
    Command { name: String, args: Option<Regex> },
}

#[derive(Debug, Clone)]
pub struct Rule {
    pub matcher: Matcher,
    pub label: String,
}

impl Rule {
    fn matches(&self, seg: &crate::analysis::Segment) -> bool {
        match &self.matcher {
            Matcher::Pattern(re) => re.is_match(&seg.text),
            Matcher::Command { name, args } => {
                if seg.wrappers.iter().any(|w| w == name) {
                    return true; // `sudo …`, `doas …` — the wrapper itself is the concern
                }
                seg.command.as_deref() == Some(name.as_str())
                    && args.as_ref().map(|re| re.is_match(&seg.args.join(" "))).unwrap_or(true)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Policy {
    pub deny: Vec<Rule>,
    pub confirm: Vec<Rule>,
    pub protected: Vec<String>,
    protected_set: Option<globset::GlobSet>,
    pub opaque: DefaultDecision,
    pub isolate_dangerous: bool,
    pub require_intent: bool,
    pub default: DefaultDecision,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "policy", rename_all = "lowercase")]
pub enum Decision {
    Deny { label: String },
    Confirm { label: String },
    Allow,
}

#[derive(Debug, thiserror::Error)]
pub enum PolicyError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("yaml: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("invalid regex in rule '{label}': {source}")]
    Regex { label: String, source: regex::Error },
}

fn compile(rules: Vec<RuleFile>) -> Result<Vec<Rule>, PolicyError> {
    rules
        .into_iter()
        .map(|r| {
            let label = r.label.clone();
            let matcher = match (r.pattern, r.command) {
                (Some(p), _) => Matcher::Pattern(Regex::new(&p).map_err(|source| PolicyError::Regex { label: label.clone(), source })?),
                (None, Some(c)) => Matcher::Command {
                    name: c,
                    args: match r.args {
                        Some(a) => Some(Regex::new(&a).map_err(|source| PolicyError::Regex { label: label.clone(), source })?),
                        None => None,
                    },
                },
                (None, None) => return Err(PolicyError::Regex { label, source: regex::Error::Syntax("rule needs `pattern` or `command`".into()) }),
            };
            Ok(Rule { matcher, label })
        })
        .collect()
}

fn compile_protected(globs: &[String]) -> Result<Option<globset::GlobSet>, PolicyError> {
    if globs.is_empty() {
        return Ok(None);
    }
    let home = dirs::home_dir();
    let mut b = globset::GlobSetBuilder::new();
    for g in globs {
        let expanded = match (&home, g.strip_prefix("~/")) {
            (Some(h), Some(rest)) => h.join(rest).display().to_string(),
            _ => g.clone(),
        };
        let glob = globset::GlobBuilder::new(&expanded)
            .literal_separator(false)
            .build()
            .map_err(|e| PolicyError::Regex { label: format!("protected {g}"), source: regex::Error::Syntax(e.to_string()) })?;
        b.add(glob);
    }
    Ok(Some(b.build().map_err(|e| PolicyError::Regex { label: "protected".into(), source: regex::Error::Syntax(e.to_string()) })?))
}

impl Policy {
    /// A plain description of the rules, for UIs that list them.
    pub fn describe(&self) -> serde_json::Value {
        let rule = |r: &Rule, kind: &str| match &r.matcher {
            Matcher::Pattern(re) => serde_json::json!({ "kind": kind, "label": r.label, "pattern": re.as_str() }),
            Matcher::Command { name, args } => serde_json::json!({ "kind": kind, "label": r.label, "command": name, "args": args.as_ref().map(|a| a.as_str()) }),
        };
        let mut rules: Vec<serde_json::Value> = self.deny.iter().map(|r| rule(r, "deny")).collect();
        rules.extend(self.confirm.iter().map(|r| rule(r, "confirm")));
        serde_json::json!({
            "rules": rules,
            "protected": self.protected,
            "opaque": self.opaque,
            "isolateDangerous": self.isolate_dangerous,
            "requireIntent": self.require_intent,
            "default": self.default,
        })
    }

    pub fn parse(yaml: &str) -> Result<Self, PolicyError> {
        let file: PolicyFile = serde_yaml::from_str(yaml)?;
        Ok(Self {
            deny: compile(file.deny)?,
            confirm: compile(file.confirm)?,
            protected_set: compile_protected(&file.protected)?,
            protected: file.protected,
            opaque: file.opaque,
            isolate_dangerous: file.isolate_dangerous,
            require_intent: file.require_intent,
            default: file.default,
        })
    }

    pub fn load(path: &Path) -> Result<Self, PolicyError> {
        Self::parse(&std::fs::read_to_string(path)?)
    }

    /// Permissive policy used only when nothing else could be loaded.
    pub fn allow_all() -> Self {
        Self { deny: vec![], confirm: vec![], protected: vec![], protected_set: None, opaque: DefaultDecision::Allow, isolate_dangerous: false, require_intent: false, default: DefaultDecision::Allow }
    }

    pub fn is_protected(&self, path: &std::path::Path) -> bool {
        self.protected_set.as_ref().map(|s| s.is_match(path)).unwrap_or(false)
    }

    /// Verdict for one segment: rules first, then opaque constructs, then default.
    pub fn evaluate_segment(&self, seg: &crate::analysis::Segment) -> Decision {
        if let Some(r) = self.deny.iter().find(|r| r.matches(seg)) {
            return Decision::Deny { label: r.label.clone() };
        }
        if let Some(r) = self.confirm.iter().find(|r| r.matches(seg)) {
            return Decision::Confirm { label: r.label.clone() };
        }
        if let Some(why) = &seg.opaque {
            return match self.opaque {
                DefaultDecision::Allow => Decision::Allow,
                DefaultDecision::Confirm => Decision::Confirm { label: format!("opaque execution · {why}") },
                DefaultDecision::Deny => Decision::Deny { label: format!("opaque execution · {why}") },
            };
        }
        match self.default {
            DefaultDecision::Allow => Decision::Allow,
            DefaultDecision::Confirm => Decision::Confirm { label: "default confirm".into() },
            DefaultDecision::Deny => Decision::Deny { label: "default deny".into() },
        }
    }

    /// Worst verdict over the line's segments (no cwd/targets; see `PolicyStore::analyse`).
    pub fn evaluate(&self, cmd: &str) -> Decision {
        let mut worst = Decision::Allow;
        for seg in crate::analysis::analyse_line(cmd) {
            match self.evaluate_segment(&seg) {
                d @ Decision::Deny { .. } => return d,
                d @ Decision::Confirm { .. } => worst = d,
                Decision::Allow => {}
            }
        }
        worst
    }
}

/// One resolved filesystem target of a dangerous segment.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TargetInfo {
    pub path: String,
    pub exists: bool,
    #[serde(rename = "isDir")]
    pub is_dir: bool,
    #[serde(rename = "gitRepo")]
    pub git_repo: bool,
    /// Entries under a directory target, capped at 2000.
    pub entries: Option<usize>,
    pub protected: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SegmentVerdict {
    pub text: String,
    pub command: Option<String>,
    pub opaque: Option<String>,
    #[serde(flatten)]
    pub decision: Decision,
    pub targets: Vec<TargetInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LineAnalysis {
    pub cwd: Option<String>,
    pub segments: Vec<SegmentVerdict>,
    /// The line's overall decision after protected paths and isolation.
    #[serde(flatten)]
    pub decision: Decision,
    /// Set when the line was denied for mixing a dangerous segment with others.
    #[serde(rename = "isolationViolation")]
    pub isolation_violation: bool,
}

fn inspect_target(path: &std::path::Path, protected: bool) -> TargetInfo {
    let meta = std::fs::symlink_metadata(path).ok();
    let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
    let entries = if is_dir {
        let mut n = 0usize;
        let mut stack = vec![path.to_path_buf()];
        'walk: while let Some(d) = stack.pop() {
            if let Ok(rd) = std::fs::read_dir(&d) {
                for e in rd.flatten() {
                    n += 1;
                    if n >= 2000 {
                        break 'walk;
                    }
                    if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                        stack.push(e.path());
                    }
                }
            }
        }
        Some(n)
    } else {
        None
    };
    TargetInfo {
        path: path.display().to_string(),
        exists: meta.is_some(),
        is_dir,
        git_repo: is_dir && path.join(".git").exists(),
        entries,
        protected,
    }
}

/// A policy plus its source file, reload state and per-session allowances.
pub struct PolicyStore {
    policy: Policy,
    path: Option<PathBuf>,
    mtime: Option<SystemTime>,
    session_allow: HashSet<String>,
}

pub enum Reload {
    Unchanged,
    Reloaded,
    /// Parsing failed; the previous policy is retained.
    Failed(PolicyError),
}

impl PolicyStore {
    pub fn from_policy(policy: Policy) -> Self {
        Self { policy, path: None, mtime: None, session_allow: HashSet::new() }
    }

    /// Load from `path`, writing the example policy first if the file does not exist.
    pub fn open(path: &Path) -> Result<Self, PolicyError> {
        if !path.exists() {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::write(path, EXAMPLE_POLICY)?;
        }
        let policy = Policy::load(path)?;
        let mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok();
        Ok(Self { policy, path: Some(path.to_path_buf()), mtime, session_allow: HashSet::new() })
    }

    pub fn policy(&self) -> &Policy {
        &self.policy
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Re-read the file if its mtime changed. Keeps the old policy on failure.
    pub fn maybe_reload(&mut self) -> Reload {
        let Some(path) = &self.path else { return Reload::Unchanged };
        let mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok();
        if mtime == self.mtime {
            return Reload::Unchanged;
        }
        self.mtime = mtime;
        match Policy::load(path) {
            Ok(p) => {
                self.policy = p;
                Reload::Reloaded
            }
            Err(e) => Reload::Failed(e),
        }
    }

    /// Promote a `confirm` label to `allow` for the rest of the session.
    pub fn allow_for_session(&mut self, label: &str) {
        self.session_allow.insert(label.to_string());
    }

    pub fn disallow_for_session(&mut self, label: &str) -> bool {
        self.session_allow.remove(label)
    }

    pub fn session_allows(&self) -> Vec<String> {
        let mut v: Vec<_> = self.session_allow.iter().cloned().collect();
        v.sort();
        v
    }

    /// Evaluate with session allowances applied. `deny` is never lifted.
    pub fn evaluate(&self, cmd: &str) -> Decision {
        self.analyse(cmd, None, None).decision
    }

    fn lift(&self, d: Decision) -> Decision {
        match d {
            Decision::Confirm { label } if self.session_allow.contains(&label) => Decision::Allow,
            other => other,
        }
    }

    /// Full structural analysis of a line: per-segment verdicts, resolved targets
    /// (when `cwd` is known), protected paths, and the isolation rule.
    pub fn analyse(&self, line: &str, cwd: Option<&std::path::Path>, home: Option<&std::path::Path>) -> LineAnalysis {
        use crate::analysis;
        let segs = analysis::analyse_line(line);
        let mut verdicts = Vec::new();
        let mut worst = Decision::Allow;
        let mut dangerous = 0usize;
        for (i, seg) in segs.iter().enumerate() {
            let mut d = self.lift(self.policy.evaluate_segment(seg));
            let mut targets = Vec::new();
            if let Some(cwd) = cwd {
                let at = analysis::cwd_before(&segs, i, cwd, home);
                for p in analysis::targets(seg, &at, home) {
                    let prot = self.policy.is_protected(&p);
                    if prot && !matches!(d, Decision::Deny { .. }) {
                        d = Decision::Deny { label: "protected path".into() };
                    }
                    targets.push(inspect_target(&p, prot));
                }
            }
            if !matches!(d, Decision::Allow) {
                dangerous += 1;
            }
            match (&worst, &d) {
                (Decision::Deny { .. }, _) => {}
                (_, Decision::Deny { .. }) => worst = d.clone(),
                (Decision::Allow, Decision::Confirm { .. }) => worst = d.clone(),
                _ => {}
            }
            verdicts.push(SegmentVerdict { text: seg.text.clone(), command: seg.command.clone(), opaque: seg.opaque.clone(), decision: d, targets });
        }
        let mut isolation_violation = false;
        if self.policy.isolate_dangerous && dangerous > 0 && segs.len() > 1 {
            isolation_violation = true;
            worst = Decision::Deny { label: "run dangerous commands alone".into() };
        }
        LineAnalysis { cwd: cwd.map(|c| c.display().to_string()), segments: verdicts, decision: worst, isolation_violation }
    }

    pub fn require_intent(&self) -> bool {
        self.policy.require_intent
    }

    /// Remote files and non-POSIX grammar are not inspected against the host filesystem.
    /// Preserve deny rules, but never claim these contexts are automatically verified.
    /// This review is per command and cannot be lifted by a session-wide allowance.
    pub fn analyse_external(&self, line: &str, shell: crate::backend::ShellKind) -> LineAnalysis {
        use crate::backend::ShellKind;
        let segments = if shell == ShellKind::Posix { crate::analysis::analyse_line(line) }
        else { external_segments(line, shell) };
        let mut verdicts = Vec::new();
        let mut worst = Decision::Confirm { label: "Review command in this shell/remote environment".into() };
        for segment in &segments {
            let mut decision = self.policy.evaluate_segment(segment);
            // PowerShell and cmd executable names are case insensitive.
            for rule in &self.policy.deny {
                if let Matcher::Command {name,args} = &rule.matcher {
                    if segment.command.as_ref().is_some_and(|c|c.eq_ignore_ascii_case(name)) && args.as_ref().map(|a|a.is_match(&segment.args.join(" "))).unwrap_or(true) {
                        decision = Decision::Deny {label:rule.label.clone()};
                    }
                }
            }
            if !matches!(decision, Decision::Deny {..}) { decision=worst.clone(); }
            if matches!(decision, Decision::Deny {..}) { worst=decision.clone(); }
            verdicts.push(SegmentVerdict {text:segment.text.clone(),command:segment.command.clone(),opaque:Some("Target syntax or filesystem requires human review".into()),decision,targets:vec![]});
        }
        let isolation_violation = self.policy.isolate_dangerous && segments.len()>1;
        if isolation_violation { worst=Decision::Deny {label:"Run commands separately in this shell/remote environment".into()}; }
        LineAnalysis {cwd:None,segments:verdicts,decision:worst,isolation_violation}
    }

}

/// Small dialect-aware lexer for explicit command rules. It does not certify scripts.
fn external_segments(line: &str, shell: crate::backend::ShellKind) -> Vec<crate::analysis::Segment> {
    use crate::backend::ShellKind;
    let escape=if shell==ShellKind::PowerShell {'`'} else {'^'};
    let mut pieces=vec![]; let mut start=0; let mut quote=None; let mut escaped=false;
    for (i,c) in line.char_indices() {
        if escaped {escaped=false;continue;}
        if c==escape {escaped=true;continue;}
        if quote==Some(c) {quote=None;continue;}
        if quote.is_none() && (c=='"' || (c=='\'' && shell!=ShellKind::Cmd)) {quote=Some(c);continue;}
        if quote.is_none() && ";&|\n".contains(c) { if !line[start..i].trim().is_empty(){pieces.push(line[start..i].trim());} start=i+c.len_utf8(); }
    }
    if !line[start..].trim().is_empty(){pieces.push(line[start..].trim());}
    pieces.into_iter().map(|text| {
        let mut argv=vec![];let mut word=String::new();let mut quote=None;let mut escaped=false;
        for c in text.chars() {
            if escaped {word.push(c);escaped=false;continue;}
            if c==escape {escaped=true;continue;}
            if quote==Some(c) {quote=None;continue;}
            if quote.is_none() && (c=='"' || (c=='\'' && shell!=ShellKind::Cmd)) {quote=Some(c);continue;}
            if quote.is_none() && c.is_whitespace() {if !word.is_empty(){argv.push(std::mem::take(&mut word));}} else {word.push(c);}
        }
        if !word.is_empty(){argv.push(word);}
        let command=argv.first().map(|c|c.rsplit(['/', '\\']).next().unwrap_or(c).trim_end_matches(".exe").to_string());
        let args=argv.iter().skip(1).cloned().collect();
        crate::analysis::Segment {text:text.into(),argv,command,args,opaque:Some("External shell".into()),wrappers:vec![]}
    }).collect()
}
