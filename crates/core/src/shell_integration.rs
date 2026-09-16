//! Shell-authored command boundaries. This channel never consumes PTY input/output.
//! An integration is an observation mechanism, not a same-user security boundary.
use serde::Serialize;
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationStatus {
    pub state: String,
    pub shell: Option<String>,
    pub reason: Option<String>,
}
impl IntegrationStatus {
    pub fn unavailable(reason: &str) -> Self {
        Self {
            state: "unavailable".into(),
            shell: None,
            reason: Some(reason.into()),
        }
    }
}

pub(crate) struct RunningCommand {
    pub id: String,
    pub actor: String,
    pub started: Instant,
    pub sequence: u64,
}
pub(crate) struct Submission {
    pub id: String,
    pub actor: String,
    pub command: String,
    pub at: Instant,
}

#[cfg(unix)]
pub(crate) struct Integration {
    directory: tempfile::TempDir,
    next: u64,
    created: Instant,
    timed_out: bool,
}

#[cfg(unix)]
impl Integration {
    pub fn prepare(
        plan: &mut crate::backend::LaunchPlan,
        env: &[(String, String)],
    ) -> std::io::Result<Option<Self>> {
        use crate::backend::quote_posix as q;
        use std::os::unix::fs::PermissionsExt;
        let shell = std::path::Path::new(&plan.program)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        if !matches!(shell, "bash" | "zsh")
            || plan.args.iter().any(|s| {
                !matches!(
                    s.as_str(),
                    "-l" | "--login" | "-i" | "-il" | "-li" | "--norc" | "--noprofile"
                )
            })
        {
            return Ok(None);
        }
        let shell = shell.to_string();
        let value = |key: &str| {
            plan.env
                .get(key)
                .cloned()
                .or_else(|| {
                    env.iter()
                        .rev()
                        .find(|(k, _)| k == key)
                        .map(|(_, v)| v.clone())
                })
                .or_else(|| std::env::var(key).ok())
        };
        let prompt = value("PROMPT_COMMAND").unwrap_or_default();
        let zdotdir = value("ZDOTDIR");
        let directory = tempfile::Builder::new().prefix("conn-shell-").tempdir()?;
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))?;
        let path = directory.path().to_string_lossy();
        let write = |name: &str, content: &str| -> std::io::Result<()> {
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(directory.path().join(name))?;
            f.write_all(content.as_bytes())
        };
        write("ack", "0\n")?;
        let common = format!(
            "[[ -z ${{__conn_loaded-}} ]] || return\n__conn_dir={}\n{}",
            q(&path),
            include_str!("shell_integration/common.sh")
        );
        if shell == "bash" {
            let bootstrap = format!(
                "__conn_prior_debug=$(trap -p DEBUG); builtin source {}",
                q(&format!("{path}/bash.sh"))
            );
            write(
                "bash.sh",
                &format!(
                    "{common}\n__conn_saved_prompt={}\n{}",
                    q(&prompt),
                    include_str!("shell_integration/bash.sh")
                ),
            )?;
            plan.env.insert(
                "PROMPT_COMMAND".into(),
                if prompt.is_empty() {
                    bootstrap
                } else {
                    format!("{prompt}\n{bootstrap}")
                },
            );
        } else {
            write(
                "zsh.sh",
                &format!("{common}\n{}", include_str!("shell_integration/zsh.sh")),
            )?;
            // Restore ZDOTDIR before normal startup continues. No user file is rewritten.
            let restore = zdotdir
                .as_ref()
                .map(|p| format!("export ZDOTDIR={}\n", q(p)))
                .unwrap_or("unset ZDOTDIR\n".into());
            write(".zshenv", &format!("{restore}\n[[ ! -f ${{ZDOTDIR:-$HOME}}/.zshenv ]] || builtin source \"${{ZDOTDIR:-$HOME}}/.zshenv\"\n__conn_boot() {{\n precmd_functions=(${{precmd_functions:#__conn_boot}})\n builtin source {}\n}}\nprecmd_functions+=(__conn_boot)\n", q(&format!("{path}/zsh.sh"))))?;
            plan.env.insert("ZDOTDIR".into(), path.into_owned());
        }
        Ok(Some(Self {
            directory,
            next: 1,
            created: Instant::now(),
            timed_out: false,
        }))
    }

    pub fn drain(&mut self, session: &mut crate::session::Session, pid: Option<u32>) {
        use std::io::Read;
        use std::os::unix::fs::OpenOptionsExt;
        for _ in 0..64 {
            let path = self.directory.path().join(format!("{}.event", self.next));
            let Ok(file) = std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
                .open(&path)
            else {
                break;
            };
            if !file.metadata().is_ok_and(|m| m.is_file()) {
                break;
            }
            let mut data = Vec::new();
            if file.take(65537).read_to_end(&mut data).is_err() {
                break;
            }
            // A writer may still be filling the file. Only complete framed records count.
            if data.len() > 65536 {
                session.shell_integration_lost("oversized_event");
            } else if data.last() != Some(&0) || data.iter().filter(|b| **b == 0).count() < 7 {
                break;
            } else if let Some((kind, text, cwd, result)) = parse(&data, pid, self.next) {
                session.shell_event(self.next, kind, text, cwd, result);
            } else {
                session.shell_integration_lost("invalid_event");
            }
            let _ = std::fs::remove_file(path);
            // The shell can only advance the bounded mailbox after this acknowledgement.
            let _ = std::fs::write(
                self.directory.path().join("ack.next"),
                format!("{}\n", self.next),
            );
            let _ = std::fs::rename(
                self.directory.path().join("ack.next"),
                self.directory.path().join("ack"),
            );
            self.next += 1;
        }
        if !self.timed_out && self.created.elapsed() > Duration::from_secs(5) {
            self.timed_out = true;
            if session.status().shell_integration.state == "starting" {
                session.shell_integration_lost("startup_hook_unavailable");
            }
        }
    }
}

#[cfg(unix)]
fn parse(data: &[u8], pid: Option<u32>, sequence: u64) -> Option<(&str, &str, &str, &str)> {
    let fields: Vec<_> = data.split(|b| *b == 0).collect();
    if fields.len() != 8 || !fields[7].is_empty() {
        return None;
    }
    let fields: Vec<_> = fields[..7]
        .iter()
        .map(|s| std::str::from_utf8(s))
        .collect::<Result<_, _>>()
        .ok()?;
    if fields[0] != "1"
        || fields[1].parse::<u32>().ok() != pid
        || fields[2].parse::<u64>().ok()? != sequence
    {
        return None;
    }
    if !matches!(fields[3], "ready" | "start" | "end" | "gap" | "unavailable") {
        return None;
    }
    Some((fields[3], fields[4], fields[5], fields[6]))
}

#[cfg(all(test, unix))]
mod tests {
    use super::parse;

    #[test]
    fn rejects_foreign_out_of_order_and_incomplete_records() {
        let record = b"1\x007\x001\x00start\x00pwd\x00/tmp\x00\x00";
        assert!(parse(record, Some(8), 1).is_none());
        assert!(parse(record, Some(7), 2).is_none());
        assert!(parse(&record[..record.len() - 1], Some(7), 1).is_none());
        assert!(parse(b"\x1b]133;C\x07", Some(7), 1).is_none());
    }

    #[test]
    fn preserves_multiline_unicode_as_one_shell_command() {
        let command = "printf '%s\\n' '한글 🚀\nsecond line'";
        let record = format!("1\x007\x001\x00start\x00{command}\x00/tmp\x00\x00");
        assert_eq!(parse(record.as_bytes(), Some(7), 1), Some(("start", command, "/tmp", "")));
    }

    #[test]
    fn gap_does_not_overflow_the_last_mailbox_slot() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("ack"), "0\n").unwrap();
        let script = format!("__conn_dir={}\n{}\n__conn_seq=63\n__conn_lost=1\n__conn_emit start pwd ''\nprintf '%s' \"$__conn_seq\"",
            crate::backend::quote_posix(directory.path().to_str().unwrap()), include_str!("shell_integration/common.sh"));
        let output = std::process::Command::new("/bin/bash").args(["-c", &script]).env_remove("BASH_ENV").output().unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, b"64");
        assert!(directory.path().join("64.event").exists());
        assert!(!directory.path().join("65.event").exists());
    }
}
