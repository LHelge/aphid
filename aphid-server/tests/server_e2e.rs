//! End-to-end tests: run the real `aphid-server` binary on an ephemeral port
//! against a temp directory and talk HTTP to it.

use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpStream},
    path::Path,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

use reqwest::{StatusCode, header};
use serde_json::Value;
use tempfile::TempDir;

struct TestServer {
    child: Child,
    addr: SocketAddr,
    lines: Arc<Mutex<Vec<String>>>,
}

impl TestServer {
    fn start(root: &Path) -> Self {
        Self::start_with(root, &["--log-format", "json"], &[])
    }

    fn start_with(root: &Path, args: &[&str], env: &[(&str, &str)]) -> Self {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_aphid-server"));
        cmd.env_remove("PORT")
            .env_remove("APHID_ROOT")
            .env_remove("APHID_LOG_FORMAT")
            .env_remove("APHID_LOG_LEVEL")
            .env_remove("RUST_LOG");
        if !env.iter().any(|(k, _)| *k == "APHID_ROOT") {
            cmd.arg("--root").arg(root);
        }
        if !env.iter().any(|(k, _)| *k == "PORT") && !args.contains(&"--port") {
            cmd.args(["--port", "0"]);
        }
        cmd.args(args).envs(env.iter().copied());
        let mut child = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("spawn aphid-server");

        let stdout = child.stdout.take().unwrap();
        let lines = Arc::new(Mutex::new(Vec::new()));
        let (tx, rx) = mpsc::channel();
        {
            let lines = Arc::clone(&lines);
            std::thread::spawn(move || {
                for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                    // Record before signalling, so a line that unblocks
                    // the caller is already visible in `lines`.
                    lines.lock().unwrap().push(line.clone());
                    let _ = tx.send(line);
                }
            });
        }

        let deadline = Instant::now() + Duration::from_secs(10);
        let port = loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let line = rx
                .recv_timeout(remaining)
                .expect("server never logged 'listening'");
            if line.contains("listening") {
                break parse_port(&line);
            }
        };
        Self {
            child,
            addr: SocketAddr::from(([127, 0, 0, 1], port)),
            lines,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.addr)
    }

    /// Send a raw HTTP/1.1 request (for paths an HTTP client would normalise).
    fn raw(&self, request_target: &str) -> (u16, String, String) {
        let mut stream = TcpStream::connect(self.addr).unwrap();
        write!(
            stream,
            "GET {request_target} HTTP/1.1\r\nHost: test\r\nConnection: close\r\n\r\n"
        )
        .unwrap();
        let mut out = String::new();
        stream.read_to_string(&mut out).unwrap();
        let (head, body) = out.split_once("\r\n\r\n").unwrap_or((&out, ""));
        let status = head.split(' ').nth(1).unwrap().parse().unwrap();
        (status, head.to_owned(), body.to_owned())
    }

    /// JSON access-log entries seen so far.
    fn access_log(&self) -> Vec<Value> {
        self.lines
            .lock()
            .unwrap()
            .iter()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .filter(|v| v["target"] == "aphid_server::access")
            .collect()
    }

    /// Wait until an access-log entry for `path` shows up.
    fn wait_for_access(&self, path: &str) -> Value {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(entry) = self.access_log().into_iter().find(|e| e["path"] == path) {
                return entry;
            }
            assert!(Instant::now() < deadline, "no access log entry for {path}");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn signal(&self, sig: &str) {
        let status = Command::new("kill")
            .args([sig, &self.child.id().to_string()])
            .status()
            .unwrap();
        assert!(status.success());
    }

    fn wait_exit(&mut self) -> std::process::ExitStatus {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status;
            }
            assert!(Instant::now() < deadline, "server did not exit");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Extract the port from the `listening` line in either log format.
fn parse_port(line: &str) -> u16 {
    let rest = &line[line.find("0.0.0.0:").expect("addr in listening line") + 8..];
    rest.chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .unwrap()
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap()
}

/// `<tmp>/site` is the root; `<tmp>/secret.txt` sits outside it.
fn site() -> (TempDir, std::path::PathBuf) {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().join("site");
    let write = |rel: &str, body: &str| {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    };
    write("index.html", "<h1>home</h1>");
    write("posts/index.html", "<h1>posts</h1>");
    write("style.css", "body{}");
    write("app.js", "console.log(1)");
    write("data.txt", "0123456789");
    write("healthz", "file, not health check");
    write("evil.com/index.html", "dir named like a host");
    std::fs::write(tmp.path().join("secret.txt"), "TOP SECRET").unwrap();
    (tmp, root)
}

fn site_with_404() -> (TempDir, std::path::PathBuf) {
    let (tmp, root) = site();
    std::fs::write(root.join("404.html"), "<h1>custom not found</h1>").unwrap();
    (tmp, root)
}

#[tokio::test]
async fn serves_index_html_for_directories() {
    let (_tmp, root) = site();
    let server = TestServer::start(&root);
    for path in ["/", "/posts/"] {
        let res = client().get(server.url(path)).send().await.unwrap();
        assert_eq!(res.status(), StatusCode::OK, "{path}");
        assert!(
            res.headers()[header::CONTENT_TYPE]
                .to_str()
                .unwrap()
                .starts_with("text/html")
        );
        assert!(res.text().await.unwrap().contains("<h1>"));
    }
}

#[tokio::test]
async fn sets_content_types() {
    let (_tmp, root) = site();
    let server = TestServer::start(&root);
    for (path, expected) in [
        ("/style.css", "text/css"),
        ("/app.js", "text/javascript"),
        ("/data.txt", "text/plain"),
    ] {
        let res = client().get(server.url(path)).send().await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let ct = res.headers()[header::CONTENT_TYPE].to_str().unwrap();
        assert!(ct.starts_with(expected), "{path}: {ct}");
    }
}

#[tokio::test]
async fn honours_etag_and_last_modified() {
    let (_tmp, root) = site();
    let server = TestServer::start(&root);
    let res = client().get(server.url("/style.css")).send().await.unwrap();
    let etag = res.headers()[header::ETAG].clone();
    let last_modified = res.headers()[header::LAST_MODIFIED].clone();

    let res = client()
        .get(server.url("/style.css"))
        .header(header::IF_NONE_MATCH, etag)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_MODIFIED);

    let res = client()
        .get(server.url("/style.css"))
        .header(header::IF_MODIFIED_SINCE, last_modified)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_MODIFIED);
}

#[tokio::test]
async fn serves_range_requests() {
    let (_tmp, root) = site();
    let server = TestServer::start(&root);
    let res = client()
        .get(server.url("/data.txt"))
        .header(header::RANGE, "bytes=2-5")
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(res.headers()[header::CONTENT_RANGE], "bytes 2-5/10");
    assert_eq!(res.text().await.unwrap(), "2345");
}

#[tokio::test]
async fn compresses_text_when_the_client_accepts_it() {
    let (_tmp, root) = site();
    let page = "<p>compress me</p>".repeat(200);
    std::fs::write(root.join("big.html"), &page).unwrap();
    let server = TestServer::start(&root);

    for (accept, encoding) in [("gzip", "gzip"), ("br", "br"), ("gzip, br", "br")] {
        let res = client()
            .get(server.url("/big.html"))
            .header(header::ACCEPT_ENCODING, accept)
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers()[header::CONTENT_ENCODING],
            encoding,
            "{accept}"
        );
        assert_eq!(res.headers()[header::VARY], "accept-encoding");
        let body = res.bytes().await.unwrap();
        assert!(
            body.len() < page.len() / 2,
            "{accept}: {} bytes",
            body.len()
        );
        if encoding == "gzip" {
            assert_eq!(body[..2], [0x1f, 0x8b]);
        }
    }

    // The access log reports the size before compression.
    assert_eq!(server.wait_for_access("/big.html")["bytes"], page.len());
}

#[tokio::test]
async fn leaves_responses_alone_without_accept_encoding() {
    let (_tmp, root) = site();
    let page = "<p>compress me</p>".repeat(200);
    std::fs::write(root.join("big.html"), &page).unwrap();
    let server = TestServer::start(&root);

    let res = client().get(server.url("/big.html")).send().await.unwrap();
    assert!(!res.headers().contains_key(header::CONTENT_ENCODING));
    assert_eq!(
        res.headers()[header::CONTENT_LENGTH],
        page.len().to_string()
    );
    assert_eq!(res.text().await.unwrap(), page);
}

#[tokio::test]
async fn does_not_compress_images_or_ranges() {
    let (_tmp, root) = site();
    let blob = "0123456789".repeat(500);
    std::fs::write(root.join("photo.png"), &blob).unwrap();
    std::fs::write(root.join("big.txt"), &blob).unwrap();
    let server = TestServer::start(&root);

    let res = client()
        .get(server.url("/photo.png"))
        .header(header::ACCEPT_ENCODING, "gzip, br")
        .send()
        .await
        .unwrap();
    assert!(!res.headers().contains_key(header::CONTENT_ENCODING));
    assert_eq!(res.bytes().await.unwrap().len(), blob.len());

    let res = client()
        .get(server.url("/big.txt"))
        .header(header::ACCEPT_ENCODING, "gzip, br")
        .header(header::RANGE, "bytes=2-5")
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::PARTIAL_CONTENT);
    assert!(!res.headers().contains_key(header::CONTENT_ENCODING));
    assert_eq!(res.text().await.unwrap(), "2345");
}

#[tokio::test]
async fn serves_precompressed_variants_when_present() {
    let (_tmp, root) = site();
    let page = "<p>precompressed</p>".repeat(200);
    std::fs::create_dir_all(root.join("docs")).unwrap();
    std::fs::write(root.join("docs/index.html"), &page).unwrap();
    // Not real brotli/gzip: the server must send the files untouched.
    std::fs::write(root.join("docs/index.html.br"), "BR VARIANT").unwrap();
    std::fs::write(root.join("docs/index.html.gz"), "GZ VARIANT").unwrap();
    let server = TestServer::start(&root);

    for (accept, encoding, body) in [
        ("br", "br", "BR VARIANT"),
        ("gzip", "gzip", "GZ VARIANT"),
        ("gzip, br", "br", "BR VARIANT"),
    ] {
        let res = client()
            .get(server.url("/docs/"))
            .header(header::ACCEPT_ENCODING, accept)
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK, "{accept}");
        assert_eq!(
            res.headers()[header::CONTENT_ENCODING],
            encoding,
            "{accept}"
        );
        assert_eq!(res.headers()[header::CONTENT_LENGTH], "10", "{accept}");
        assert!(
            res.headers()[header::CONTENT_TYPE]
                .to_str()
                .unwrap()
                .starts_with("text/html")
        );
        assert!(
            res.headers()[header::VARY]
                .to_str()
                .unwrap()
                .eq_ignore_ascii_case("accept-encoding")
        );
        assert_eq!(res.text().await.unwrap(), body, "{accept}");
    }

    // The access log reports the size of the file sent.
    assert_eq!(server.wait_for_access("/docs/")["bytes"], 10);

    // A client that accepts no encoding gets the original file.
    let res = client().get(server.url("/docs/")).send().await.unwrap();
    assert!(!res.headers().contains_key(header::CONTENT_ENCODING));
    assert_eq!(res.text().await.unwrap(), page);
}

#[tokio::test]
async fn compresses_on_the_fly_when_a_variant_is_missing() {
    let (_tmp, root) = site();
    let page = "<p>only gzip on disk</p>".repeat(200);
    std::fs::write(root.join("big.html"), &page).unwrap();
    std::fs::write(root.join("big.html.gz"), "GZ VARIANT").unwrap();
    let server = TestServer::start(&root);

    let res = client()
        .get(server.url("/big.html"))
        .header(header::ACCEPT_ENCODING, "br")
        .send()
        .await
        .unwrap();
    assert_eq!(res.headers()[header::CONTENT_ENCODING], "br");
    assert!(!res.headers().contains_key(header::CONTENT_LENGTH));
    let body = res.bytes().await.unwrap();
    assert!(body.len() < page.len() / 2);
    assert_ne!(&body[..], b"GZ VARIANT");
}

#[tokio::test]
async fn redirects_directories_with_relative_location() {
    let (_tmp, root) = site();
    let server = TestServer::start(&root);
    for (path, location) in [("/posts", "/posts/"), ("/posts?page=2", "/posts/?page=2")] {
        let res = client().get(server.url(path)).send().await.unwrap();
        assert!(res.status().is_redirection(), "{path}: {}", res.status());
        assert_eq!(res.headers()[header::LOCATION], location);
    }
}

#[tokio::test]
async fn redirect_never_becomes_protocol_relative() {
    let (_tmp, root) = site();
    let server = TestServer::start(&root);
    let (status, head, _) = server.raw("//evil.com");
    assert!((300..400).contains(&status), "{head}");
    assert!(head.contains("location: /evil.com/\r\n"), "{head}");
}

#[tokio::test]
async fn missing_path_serves_404_html() {
    let (_tmp, root) = site_with_404();
    let server = TestServer::start(&root);
    let res = client()
        .get(server.url("/nope/"))
        .header(header::RANGE, "bytes=0-1")
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert!(
        res.headers()[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("text/html")
    );
    assert_eq!(res.text().await.unwrap(), "<h1>custom not found</h1>");
}

#[tokio::test]
async fn missing_path_without_404_html_is_plain() {
    let (_tmp, root) = site();
    let server = TestServer::start(&root);
    let res = client().get(server.url("/nope.png")).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(res.text().await.unwrap(), "404 Not Found\n");
}

#[tokio::test]
async fn healthz_wins_over_file_and_is_not_logged() {
    let (_tmp, root) = site();
    let server = TestServer::start(&root);
    let res = client().get(server.url("/healthz")).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await.unwrap(), "");

    // A logged request after it acts as a barrier for the log reader.
    client().get(server.url("/")).send().await.unwrap();
    server.wait_for_access("/");
    assert!(server.access_log().iter().all(|e| e["path"] != "/healthz"));
}

#[tokio::test]
async fn healthz_works_without_site_files() {
    let tmp = TempDir::new().unwrap();
    let server = TestServer::start(tmp.path());
    std::fs::remove_dir(tmp.path()).unwrap();
    let res = client().get(server.url("/healthz")).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn traversal_cannot_escape_root() {
    let (_tmp, root) = site_with_404();
    let server = TestServer::start(&root);
    for target in [
        "/../secret.txt",
        "/%2e%2e/secret.txt",
        "/posts/../../secret.txt",
        "/posts/%2e%2e%2f%2e%2e%2fsecret.txt",
        "/..%5csecret.txt",
        "/%2fetc%2fpasswd",
    ] {
        let (status, head, body) = server.raw(target);
        assert!(status == 404 || status == 400, "{target}: {head}");
        assert!(!body.contains("TOP SECRET"), "{target} leaked the file");
    }
}

#[tokio::test]
async fn logs_one_event_per_request() {
    let (_tmp, root) = site();
    let server = TestServer::start(&root);
    client()
        .get(server.url("/missing-page/"))
        .header("x-forwarded-for", "203.0.113.7, 10.0.0.1")
        .header(header::REFERER, "https://example.com/posts/")
        .header(header::USER_AGENT, "test-agent/1.0")
        .send()
        .await
        .unwrap();
    client().get(server.url("/data.txt")).send().await.unwrap();

    let missing = server.wait_for_access("/missing-page/");
    assert_eq!(missing["level"], "INFO");
    assert_eq!(missing["method"], "GET");
    assert_eq!(missing["status"], 404);
    assert_eq!(missing["referrer"], "https://example.com/posts/");
    assert_eq!(missing["user_agent"], "test-agent/1.0");
    assert_eq!(missing["client"], "203.0.113.7");
    assert!(missing["duration_ms"].as_f64().unwrap() >= 0.0);

    let ok = server.wait_for_access("/data.txt");
    assert_eq!(ok["status"], 200);
    assert_eq!(ok["bytes"], 10);
    assert_eq!(ok["client"], "127.0.0.1");

    let count = server
        .access_log()
        .iter()
        .filter(|e| e["path"] == "/data.txt")
        .count();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn reads_config_from_env_and_flags_win() {
    let (_tmp, root) = site();
    let root_str = root.to_str().unwrap();

    // Everything from the environment.
    let server = TestServer::start_with(
        &root,
        &[],
        &[
            ("APHID_ROOT", root_str),
            ("PORT", "0"),
            ("APHID_LOG_FORMAT", "json"),
        ],
    );
    let res = client().get(server.url("/data.txt")).send().await.unwrap();
    assert_eq!(res.text().await.unwrap(), "0123456789");
    server.wait_for_access("/data.txt");
    drop(server);

    // Flags override the environment (PORT here would not even parse).
    let server = TestServer::start_with(
        &root,
        &["--port", "0", "--log-format", "json"],
        &[("PORT", "not-a-port"), ("APHID_LOG_FORMAT", "text")],
    );
    client().get(server.url("/")).send().await.unwrap();
    server.wait_for_access("/");
}

#[tokio::test]
async fn text_log_format_is_default() {
    let (_tmp, root) = site();
    let server = TestServer::start_with(&root, &[], &[]);
    let lines = server.lines.lock().unwrap().clone();
    assert!(lines.iter().any(|l| l.contains("listening")));
    assert!(
        lines
            .iter()
            .all(|l| serde_json::from_str::<Value>(l).is_err())
    );
}

#[test]
fn missing_root_fails_at_startup() {
    let tmp = TempDir::new().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_aphid-server"))
        .arg("--root")
        .arg(tmp.path().join("nope"))
        .args(["--port", "0"])
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
fn log_format_none_still_reports_fatal_errors_on_stderr() {
    let tmp = TempDir::new().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_aphid-server"))
        .arg("--root")
        .arg(tmp.path().join("nope"))
        .args(["--port", "0", "--log-format", "none"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty(), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.starts_with("aphid-server: "), "{stderr}");
}

#[cfg(unix)]
#[test]
fn graceful_shutdown_finishes_in_flight_requests() {
    let (_tmp, root) = site();
    let size = 32 * 1024 * 1024;
    std::fs::write(root.join("big.bin"), vec![b'x'; size]).unwrap();
    let mut server = TestServer::start(&root);

    // Start a large download and stop reading, so the response is in flight.
    let mut stream = TcpStream::connect(server.addr).unwrap();
    write!(
        stream,
        "GET /big.bin HTTP/1.1\r\nHost: test\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut first = [0u8; 1024];
    stream.read_exact(&mut first).unwrap();

    server.signal("-TERM");

    // New connections are refused once the listener is closed.
    let deadline = Instant::now() + Duration::from_secs(5);
    while TcpStream::connect(server.addr).is_ok() {
        assert!(Instant::now() < deadline, "still accepting after SIGTERM");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        server.child.try_wait().unwrap().is_none(),
        "exited before draining"
    );

    // The in-flight response still completes.
    let mut rest = Vec::new();
    stream.read_to_end(&mut rest).unwrap();
    let all = [&first[..], &rest[..]].concat();
    let header_end = all.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
    assert_eq!(all.len() - header_end, size);

    assert!(server.wait_exit().success());
}

#[cfg(unix)]
#[test]
fn sigint_exits_zero() {
    let (_tmp, root) = site();
    let mut server = TestServer::start(&root);
    server.signal("-INT");
    assert!(server.wait_exit().success());
}
