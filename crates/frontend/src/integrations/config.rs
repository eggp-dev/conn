//! Lossless edits to the Conn entry only. Never normalize an entire user config.
use super::adapters::Format;
use jsonc_parser::cst::{CstInputValue, CstObject, CstRootNode};
use serde_json::Value;
use toml_edit::{DocumentMut, Item, Table};

pub fn read(text: &str, format: Format) -> Result<Option<Value>, String> {
    let root: Value = match format {
        Format::Toml => toml_edit::de::from_str(text).map_err(|_| "invalid_config")?,
        Format::Json(key) => {
            let root =
                CstRootNode::parse(text, &Default::default()).map_err(|_| "invalid_config")?;
            let object = root.object_value().ok_or("invalid_config")?;
            unique(&object, key)?;
            if let Some(servers) = object.object_value(key) {
                unique(&servers, "conn")?;
            }
            jsonc_parser::parse_to_serde_value(text, &Default::default())
                .map_err(|_| "invalid_config")?
        }
    };
    let key = match format {
        Format::Toml => "mcp_servers",
        Format::Json(key) => key,
    };
    let object = root.as_object().ok_or("invalid_config")?;
    let Some(servers) = object.get(key) else {
        return Ok(None);
    };
    Ok(servers
        .as_object()
        .ok_or("invalid_config")?
        .get("conn")
        .cloned())
}
fn unique(object: &CstObject, key: &str) -> Result<(), String> {
    // JSON permits ambiguous duplicate names; different clients may pick different values.
    let count = object
        .properties()
        .iter()
        .filter(|p| p.name().and_then(|n| n.decoded_value().ok()).as_deref() == Some(key))
        .count();
    if count > 1 {
        Err("invalid_config".into())
    } else {
        Ok(())
    }
}
fn input(v: &Value) -> CstInputValue {
    match v {
        Value::Null => CstInputValue::Null,
        Value::Bool(v) => CstInputValue::Bool(*v),
        Value::Number(v) => CstInputValue::Number(v.to_string()),
        Value::String(v) => CstInputValue::String(v.clone()),
        Value::Array(v) => CstInputValue::Array(v.iter().map(input).collect()),
        Value::Object(v) => {
            CstInputValue::Object(v.iter().map(|(k, v)| (k.clone(), input(v))).collect())
        }
    }
}
pub fn edit(text: &str, format: Format, entry: Option<&Value>) -> Result<String, String> {
    read(text, format)?;
    match format {
        Format::Json(key) => {
            let root =
                CstRootNode::parse(text, &Default::default()).map_err(|_| "invalid_config")?;
            let object = root.object_value().ok_or("invalid_config")?;
            if let Some(entry) = entry {
                let servers = object.object_value_or_create(key).ok_or("invalid_config")?;
                match servers.get("conn") {
                    Some(p) => p.set_value(input(entry)),
                    None => {
                        servers.append("conn", input(entry));
                    }
                }
            } else if let Some(p) = object.object_value(key).and_then(|s| s.get("conn")) {
                p.remove();
            }
            Ok(root.to_string())
        }
        Format::Toml => {
            let mut doc: DocumentMut = text.parse().map_err(|_| "invalid_config")?;
            if let Some(entry) = entry {
                // Deserializing the generated entry supports proper escaping on Windows.
                let mut table = Table::new();
                for (key, value) in entry.as_object().ok_or("invalid_config")? {
                    let parsed: DocumentMut = format!("value = {value}\n")
                        .parse()
                        .map_err(|_| "invalid_config")?;
                    table.insert(key, parsed["value"].clone());
                }
                if doc.get("mcp_servers").is_none() {
                    doc["mcp_servers"] = Item::Table(Table::new());
                }
                let servers = doc["mcp_servers"]
                    .as_table_like_mut()
                    .ok_or("invalid_config")?;
                servers.insert("conn", Item::Table(table));
            } else if let Some(servers) =
                doc.get_mut("mcp_servers").and_then(Item::as_table_like_mut)
            {
                servers.remove("conn");
            }
            Ok(doc.to_string())
        }
    }
}
pub fn empty(format: Format) -> &'static str {
    match format {
        Format::Toml => "",
        Format::Json(_) => "{\n}\n",
    }
}
