//! End-to-end OpenTelemetry tests: run the real binary against a fake OTLP
//! collector, an unreachable port, and a port that never answers.

use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::Path,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

use tempfile::TempDir;

#[derive(Debug, Clone)]
struct Received {
    path: String,
    content_type: String,
    api_key: Option<String>,
    body_len: usize,
}

/// Minimal HTTP/1.1 sink that records OTLP export requests and answers 200.
struct FakeCollector {
    addr: SocketAddr,
    received: Arc<Mutex<Vec<Received>>>,
}

impl FakeCollector {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let received = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&received);
        std::thread::spawn(move || {
            for stream in listener.incoming().map_while(Result::ok) {
                let sink = Arc::clone(&sink);
                std::thread::spawn(move || Self::serve(stream, &sink));
            }
        });
        Self { addr, received }
    }

    fn serve(stream: TcpStream, sink: &Mutex<Vec<Received>>) {
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut writer = stream;
        loop {
            let mut request_line = String::new();
            if reader.read_line(&mut request_line).unwrap_or(0) == 0 {
                return;
            }
            let path = request_line.split(' ').nth(1).unwrap_or("").to_owned();
            let (mut len, mut content_type, mut api_key) = (0, String::new(), None);
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let line = line.trim_end();
                if line.is_empty() {
                    break;
                }
                let (name, value) = line.split_once(':').unwrap();
                match name.to_ascii_lowercase().as_str() {
                    "content-length" => len = value.trim().parse().unwrap(),
                    "content-type" => content_type = value.trim().to_owned(),
                    "x-api-key" => api_key = Some(value.trim().to_owned()),
                    _ => {}
                }
            }
            let mut body = vec![0; len];
            reader.read_exact(&mut body).unwrap();
            sink.lock().unwrap().push(Received {
                path,
                content_type,
                api_key,
                body_len: len,
            });
            writer
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n")
                .unwrap();
        }
    }

    fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    fn received(&self) -> Vec<Received> {
        self.received.lock().unwrap().clone()
    }
}

struct Server {
    child: Child,
    addr: SocketAddr,
    lines: Arc<Mutex<Vec<String>>>,
}

impl Server {
    fn start(root: &Path, env: &[(&str, &str)]) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_aphid-server"))
            .env_clear()
            .envs(env.iter().copied())
            .arg("--root")
            .arg(root)
            .args(["--port", "0", "--log-format", "json"])
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let lines = Arc::new(Mutex::new(Vec::new()));
        let (tx, rx) = mpsc::channel();
        {
            let lines = Arc::clone(&lines);
            std::thread::spawn(move || {
                for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                    let _ = tx.send(line.clone());
                    lines.lock().unwrap().push(line);
                }
            });
        }
        let port = loop {
            let line = rx.recv_timeout(Duration::from_secs(10)).unwrap();
            if line.contains("listening") {
                let rest = &line[line.find("0.0.0.0:").unwrap() + 8..];
                break rest
                    .chars()
                    .take_while(char::is_ascii_digit)
                    .collect::<String>()
                    .parse()
                    .unwrap();
            }
        };
        Self {
            child,
            addr: SocketAddr::from(([127, 0, 0, 1], port)),
            lines,
        }
    }

    /// GET `path`, returning the status and how long the response took.
    fn get(&self, path: &str) -> (u16, Duration) {
        let start = Instant::now();
        let mut stream = TcpStream::connect(self.addr).unwrap();
        write!(
            stream,
            "GET {path} HTTP/1.1\r\nHost: t\r\ntraceparent: 00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01\r\nConnection: close\r\n\r\n"
        )
        .unwrap();
        let mut out = String::new();
        stream.read_to_string(&mut out).unwrap();
        let status = out.split(' ').nth(1).unwrap().parse().unwrap();
        (status, start.elapsed())
    }

    /// SIGTERM, then wait for exit; returns (success, time to exit).
    fn terminate(&mut self) -> (bool, Duration) {
        let start = Instant::now();
        Command::new("kill")
            .args(["-TERM", &self.child.id().to_string()])
            .status()
            .unwrap();
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                return (status.success(), start.elapsed());
            }
            assert!(start.elapsed() < Duration::from_secs(20), "did not exit");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn lines(&self) -> Vec<String> {
        self.lines.lock().unwrap().clone()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn site() -> TempDir {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("index.html"), "<h1>hi</h1>").unwrap();
    tmp
}

#[test]
fn exports_traces_and_logs_and_flushes_on_shutdown() {
    let collector = FakeCollector::start();
    let site = site();
    let url = collector.url();
    let mut server = Server::start(
        site.path(),
        &[
            ("OTEL_EXPORTER_OTLP_ENDPOINT", &url),
            ("OTEL_EXPORTER_OTLP_HEADERS", "x-api-key=secret%20value"),
        ],
    );
    assert_eq!(server.get("/").0, 200);
    assert_eq!(server.get("/missing").0, 404);

    // Batch delay is 5 s for spans; exiting now proves the shutdown flush.
    let (ok, took) = server.terminate();
    assert!(ok);
    assert!(took < Duration::from_secs(4), "shutdown took {took:?}");

    let received = collector.received();
    for path in ["/v1/traces", "/v1/logs"] {
        let hit = received
            .iter()
            .find(|r| r.path == path)
            .unwrap_or_else(|| panic!("nothing posted to {path}: {received:?}"));
        assert_eq!(hit.content_type, "application/x-protobuf");
        assert_eq!(hit.api_key.as_deref(), Some("secret value"));
        assert!(hit.body_len > 0);
    }
}

#[test]
fn no_endpoint_means_no_exporter() {
    let site = site();
    let mut server = Server::start(site.path(), &[]);
    server.get("/");
    assert!(server.terminate().0);
    assert!(
        !server.lines().iter().any(|l| l.contains("OpenTelemetry")),
        "{:?}",
        server.lines()
    );
}

fn assert_quiet_and_fast(endpoint: &str) {
    let site = site();
    let mut server = Server::start(site.path(), &[("OTEL_EXPORTER_OTLP_ENDPOINT", endpoint)]);

    let deadline = Instant::now() + Duration::from_millis(2500);
    let mut slowest = Duration::ZERO;
    while Instant::now() < deadline {
        let (status, took) = server.get("/");
        assert_eq!(status, 200);
        slowest = slowest.max(took);
        std::thread::sleep(Duration::from_millis(25));
    }
    assert!(
        slowest < Duration::from_millis(500),
        "slowest request {slowest:?}"
    );

    let (ok, took) = server.terminate();
    assert!(ok, "non-zero exit with a failing endpoint");
    assert!(took < Duration::from_secs(6), "shutdown took {took:?}");

    let lines = server.lines();
    let failures = lines
        .iter()
        .filter(|l| l.contains("OTLP export failing"))
        .count();
    assert!(failures <= 2, "one warning per signal at most: {lines:#?}");
    assert!(
        !lines
            .iter()
            .any(|l| l.contains("\"target\":\"opentelemetry")),
        "SDK internals leaked to stdout: {lines:#?}"
    );
}

#[test]
fn unreachable_endpoint_never_blocks_or_spams() {
    // Bind and drop to get a port that refuses connections.
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    assert_quiet_and_fast(&format!("http://127.0.0.1:{port}"));
}

#[test]
fn black_hole_endpoint_never_blocks_or_spams() {
    // Accepts connections and never answers.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        let mut held = Vec::new();
        for stream in listener.incoming().map_while(Result::ok) {
            held.push(stream);
        }
    });
    assert_quiet_and_fast(&format!("http://{addr}"));
}
