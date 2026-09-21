//! Test-only JSON-lines bridge for comparing the core grid with the UI parser.
use std::io::{self, BufRead};
use conn_core::screen::ScreenModel;
use serde_json::{json, Value};
fn main() {
    let mut screen=ScreenModel::new(24,80);
    for line in io::stdin().lock().lines() {
        let v:Value=serde_json::from_str(&line.unwrap()).unwrap();
        if v["reset"]==true { screen=ScreenModel::new(v["rows"].as_u64().unwrap() as u16,v["cols"].as_u64().unwrap() as u16); }
        else if let Some(text)=v["text"].as_str() {screen.process(text.as_bytes());}
        else {screen.resize(v["rows"].as_u64().unwrap() as u16,v["cols"].as_u64().unwrap() as u16);}
        println!("{}",json!({"screen":screen.rows(),"cursor":screen.cursor()}));
    }
}
