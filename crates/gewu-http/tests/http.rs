//! HTTP transport smoke tests against a loopback server on port 0.

use std::{
    fs,
    io::{Read, Write},
    net::TcpStream,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

use gewu_http::{ServeOptions, Server};

fn copy_showcase() -> tempfile::TempDir {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/ui/showcase");
    let temp = tempfile::tempdir().expect("tempdir");
    copy_dir(&source, temp.path());
    temp
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("mkdir");
    for entry in fs::read_dir(from).expect("read") {
        let entry = entry.expect("entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("copy");
        }
    }
}

fn static_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("static");
    fs::write(dir.path().join("index.html"), "<html>gewu</html>").expect("index");
    fs::write(dir.path().join("app.js"), "window.__gewu = {};").expect("js");
    dir
}

struct RunningServer {
    addr: String,
    stopper: std::sync::Arc<std::sync::atomic::AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl RunningServer {
    fn start(root: PathBuf, static_dir: PathBuf) -> Self {
        let server = Server::start(ServeOptions::new(root, static_dir, 0)).expect("start");
        let addr = server.local_addr().to_string();
        let stopper = server.stopper();
        let handle = thread::spawn(move || {
            let _ = server.run();
        });
        thread::sleep(Duration::from_millis(50));
        Self {
            addr,
            stopper,
            handle: Some(handle),
        }
    }
}

impl Drop for RunningServer {
    fn drop(&mut self) {
        self.stopper
            .store(true, std::sync::atomic::Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn http_get(addr: &str, path: &str) -> (u16, String, String) {
    request(addr, "GET", path, None)
}

fn http_post(addr: &str, path: &str, body: &str) -> (u16, String, String) {
    request(addr, "POST", path, Some(body))
}

fn request(addr: &str, method: &str, path: &str, body: Option<&str>) -> (u16, String, String) {
    let mut stream = TcpStream::connect(addr).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("timeout");
    let body = body.unwrap_or("");
    let request = format!(
        "{method} {path} HTTP/1.0\r\nHost: {addr}\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).expect("write");
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).expect("read");
    let response = String::from_utf8_lossy(&raw);
    let (header, payload) = response.split_once("\r\n\r\n").expect("headers");
    let status: u16 = header
        .lines()
        .next()
        .expect("status")
        .split_whitespace()
        .nth(1)
        .expect("code")
        .parse()
        .expect("status code");
    let content_type = header
        .lines()
        .find_map(|line| {
            let lower = line.to_ascii_lowercase();
            lower
                .strip_prefix("content-type:")
                .map(|value| value.trim().to_owned())
        })
        .unwrap_or_default();
    (status, content_type, payload.to_owned())
}

#[test]
fn health_snapshot_dispatch_and_static() {
    let workspace = copy_showcase();
    let assets = static_dir();
    let server = RunningServer::start(workspace.path().to_path_buf(), assets.path().to_path_buf());
    let addr = &server.addr;

    let (status, _, body) = http_get(addr, "/api/health");
    assert_eq!(status, 200);
    assert!(body.contains("\"ok\":true"));

    let (status, content_type, body) = http_get(addr, "/api/snapshot");
    assert_eq!(status, 200);
    assert!(content_type.contains("application/json"));
    assert!(body.contains("review-ui-showcase"));

    let (status, _, body) = http_post(
        addr,
        "/api/dispatch",
        r#"{"type":"visit","chunk_id":"showcase-ts-greet"}"#,
    );
    assert_eq!(status, 200);
    assert!(body.contains("showcase-ts-greet"));

    let (status, _, body) = http_post(
        addr,
        "/api/dispatch",
        r#"{"type":"visit","chunk_id":"missing"}"#,
    );
    assert_eq!(status, 400);
    assert!(body.contains("\"error\""));

    let (status, content_type, body) = http_get(addr, "/app.js");
    assert_eq!(status, 200);
    assert!(content_type.contains("javascript"));
    assert!(body.contains("__gewu"));

    let (status, content_type, body) = http_get(addr, "/unknown/route");
    assert_eq!(status, 200);
    assert!(content_type.contains("text/html"));
    assert!(body.contains("gewu"));

    let (status, _, _) = http_get(addr, "/../../etc/passwd");
    assert_eq!(
        status, 200,
        "traversal falls back to index rather than escaping"
    );
}
