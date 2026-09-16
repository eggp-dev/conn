//! Append-only JSONL audit log. Records actions, never terminal content.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub ts: String,
    pub actor: String,
    pub action: String,
    #[serde(flatten)]
    pub fields: Map<String, Value>,
}

enum Sink {
    File(Mutex<File>),
    Memory(Arc<Mutex<Vec<Event>>>),
    Null,
}

pub struct Audit {
    sink: Sink,
}

impl Audit {
    pub fn open(path: &Path) -> std::io::Result<Self> {
        let mut options = OpenOptions::new();
        options.create(true).append(true);
        #[cfg(unix)] {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600).custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
        }
        let file = options.open(path)?;
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(Self { sink: Sink::File(Mutex::new(file)) })
    }

    /// In-memory sink for tests. The returned handle can be inspected afterwards.
    pub fn memory() -> (Self, Arc<Mutex<Vec<Event>>>) {
        let store = Arc::new(Mutex::new(Vec::new()));
        (Self { sink: Sink::Memory(store.clone()) }, store)
    }

    pub fn null() -> Self {
        Self { sink: Sink::Null }
    }

    pub fn record(&self, actor: &str, action: &str, fields: Value) {
        if matches!(self.sink, Sink::Null) { return; }
        let fields = match fields {
            Value::Object(m) => m,
            Value::Null => Map::new(),
            other => {
                let mut m = Map::new();
                m.insert("detail".into(), other);
                m
            }
        };
        let event = Event {
            ts: chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
            actor: actor.to_string(),
            action: action.to_string(),
            fields,
        };
        match &self.sink {
            Sink::File(f) => {
                if let Ok(line) = serde_json::to_string(&event) {
                    if let Ok(mut f) = f.lock() {
                        let _ = writeln!(f, "{line}");
                        let _ = f.flush();
                    }
                }
            }
            Sink::Memory(store) => {
                if let Ok(mut s) = store.lock() {
                    s.push(event);
                }
            }
            Sink::Null => {}
        }
    }
}

/// Read every parseable event from an audit file.
pub fn read_events(path: &Path) -> std::io::Result<Vec<Event>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut out = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(ev) = serde_json::from_str::<Event>(&line) {
            out.push(ev);
        }
    }
    Ok(out)
}
