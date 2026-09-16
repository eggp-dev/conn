//! Exercise the real updater against a loopback fixture. Never installs a payload.
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::json;
use std::{
    io::{Read, Write},
    net::TcpListener,
    time::Duration,
};
use tauri_plugin_updater::UpdaterExt;

// Public minisign-verify test vector (MIT), message b"test". No private key.
const PUBLIC: &str = "untrusted comment: public test key\nRWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3\n";
const SIGNATURE: &str = "untrusted comment: signature from minisign secret key\nRUQf6LRCGA9i559r3g7V1qNyJDApGip8MfqcadIgT9CuhV3EMhHoN1mGTkUidF/z7SrlQgXdy8ofjb7bNJJylDOocrCo8KLzZwo=\ntrusted comment: timestamp:1556193335\tfile:test\ny/rUw2y8/hOUYjZU71eHp/Wo1KZ40fGy2VJEDl34XMJM+TX48Ss/17u3IvIfbVR1FkZZSNCisQbuQY+bHwhEBg==\n";

fn fixture(
    body: &'static [u8],
    length: usize,
    status: &str,
    metadata: bool,
) -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let endpoint = url.clone();
    let status = status.to_owned();
    let thread = std::thread::spawn(move || {
        for i in 0..if metadata { 2 } else { 1 } {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = [0; 4096];
            let _ = stream.read(&mut request).unwrap();
            let (payload, count, code) = if i == 0 && metadata {
                let bytes = json!({"version":"99.0.0","url":format!("{url}/payload"),"signature":STANDARD.encode(SIGNATURE)}).to_string().into_bytes();
                let len = bytes.len();
                (bytes, len, "200 OK")
            } else {
                (body.to_vec(), length, status.as_str())
            };
            write!(stream,"HTTP/1.1 {code}\r\nContent-Length: {count}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n").unwrap();
            stream.write_all(&payload).unwrap();
        }
    });
    (endpoint, thread)
}

fn client(
    endpoint: &str,
) -> (
    tauri::App<tauri::test::MockRuntime>,
    tauri_plugin_updater::Updater,
) {
    let mut context = tauri::test::mock_context(tauri::test::noop_assets());
    context.config_mut().plugins.0.insert(
        "updater".into(),
        json!({"pubkey":STANDARD.encode(PUBLIC),"dangerousInsecureTransportProtocol":true}),
    );
    let app = tauri::test::mock_builder()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .build(context)
        .unwrap();
    let updater = app
        .updater_builder()
        .executable_path(std::path::PathBuf::from(
            "/tmp/Conn.app/Contents/MacOS/Conn",
        ))
        .endpoints(vec![endpoint.parse().unwrap()])
        .unwrap()
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap();
    (app, updater)
}

#[test]
fn downloads_require_complete_bytes_and_a_valid_signature() {
    for (body, length, status, expected) in [
        (b"test".as_slice(), 4, "200 OK", true),
        (b"evil".as_slice(), 4, "200 OK", false),
        (b"te".as_slice(), 4, "200 OK", false),
        (b"gone".as_slice(), 4, "404 Not Found", false),
    ] {
        let (url, server) = fixture(body, length, status, true);
        let (_app, updater) = client(&url);
        tauri::async_runtime::block_on(async {
            let update = updater.check().await.unwrap().unwrap();
            let result = update.download(|_, _| {}, || {}).await;
            assert_eq!(result.is_ok(), expected, "status={status}, length={length}");
            if expected {
                assert_eq!(result.unwrap(), b"test");
            }
        });
        server.join().unwrap();
    }
}

#[test]
fn offline_or_missing_metadata_never_offers_installation() {
    let (url, server) = fixture(b"gone", 4, "404 Not Found", false);
    let (_app, updater) = client(&url);
    assert!(tauri::async_runtime::block_on(updater.check()).is_err());
    server.join().unwrap();
    // The listener is now closed, so the same endpoint is offline.
    assert!(tauri::async_runtime::block_on(updater.check()).is_err());
}

#[test]
#[ignore = "Release CI supplies final artifacts and the embedded public key"]
fn release_payload_signatures_match_the_embedded_key() {
    let directory = std::path::PathBuf::from(
        std::env::var_os("CONN_UPDATER_ARTIFACTS").expect("release artifact directory"),
    );
    let encoded = std::env::var("TAURI_SIGNING_PUBLIC_KEY").expect("release public key");
    let public = String::from_utf8(STANDARD.decode(encoded.trim()).unwrap()).unwrap();
    let key = minisign_verify::PublicKey::decode(&public).unwrap();
    let signatures: Vec<_> = std::fs::read_dir(&directory)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "sig"))
        .collect();
    assert_eq!(
        signatures.len(),
        1,
        "One platform signature is required on each runner"
    );
    for path in signatures {
        let payload = path.with_file_name(
            path.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .strip_suffix(".sig")
                .unwrap(),
        );
        let signature = String::from_utf8(
            STANDARD
                .decode(std::fs::read_to_string(path).unwrap().trim())
                .unwrap(),
        )
        .unwrap();
        key.verify(
            &std::fs::read(payload).unwrap(),
            &minisign_verify::Signature::decode(&signature).unwrap(),
            false,
        )
        .expect("Final release signature must verify");
    }
}
