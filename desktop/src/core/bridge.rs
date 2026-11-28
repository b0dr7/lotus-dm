
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub const DEFAULT_PORT: u16 = 8460;

const MAX_HEADER: usize = 16_384;
const MAX_BODY: usize = 65_536;

#[derive(Debug)]
pub struct AddRequest {
    pub url: String,
    pub filename: Option<String>,
    pub quality: Option<String>,
}

#[derive(serde::Deserialize)]
struct AddBody {
    url: String,
    #[serde(default)]
    filename: Option<String>,
    #[serde(default)]
    quality: Option<String>,
}

pub fn start(
    preferred_port: u16,
    tx: Sender<AddRequest>,
    cmd_tx: Sender<BridgeCmd>,
    wake: Arc<dyn Fn() + Send + Sync + 'static>,
) -> Option<u16> {
    for offset in 0..10u16 {
        let port = preferred_port.max(1024).saturating_add(offset);
        let Ok(listener) = TcpListener::bind(("127.0.0.1", port)) else {
            continue;
        };
        let tx = tx.clone();
        let cmd_tx = cmd_tx.clone();
        let wake = Arc::clone(&wake);
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let tx = tx.clone();
                let cmd_tx = cmd_tx.clone();
                let wake = Arc::clone(&wake);
                thread::spawn(move || {
                    if serve(stream, &tx, &cmd_tx) {
                        (wake)();
                    }
                });
            }
        });
        return Some(port);
    }
    None
}

#[derive(serde::Serialize)]
pub struct BridgeJob {
    pub id: String,
    pub url: String,
    pub filename: String,
    pub status: String,
    pub received: u64,
    pub total: u64,
    pub speed: u64,
}

pub enum BridgeCmd {
    List { reply: Sender<Vec<BridgeJob>> },
    Pause { id: String, reply: Sender<bool> },
    Resume { id: String, reply: Sender<bool> },
    Cancel { id: String, reply: Sender<bool> },
}

fn serve(mut stream: TcpStream, tx: &Sender<AddRequest>, cmd_tx: &Sender<BridgeCmd>) -> bool {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));

    let Some(head) = read_headers(&mut stream) else {
        return false;
    };

    let text = String::from_utf8_lossy(&head.head).into_owned();
    let mut lines = text.split("\r\n");
    let Some(request_line) = lines.next() else {
        respond(&mut stream, 400, "{\"ok\":false}");
        return false;
    };
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_uppercase();
    let target = parts.next().unwrap_or("/").to_string();

    let mut content_length = 0usize;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.trim().eq_ignore_ascii_case("content-length") {
            content_length = value.trim().parse().unwrap_or(0);
        }
    }
    if content_length > MAX_BODY {
        respond(&mut stream, 413, "{\"ok\":false,\"error\":\"body too large\"}");
        return false;
    }

    let body = read_body(&mut stream, head.rest, content_length);

    if method == "OPTIONS" {
        respond(&mut stream, 204, "");
        return false;
    }

    let (path, query) = match target.split_once('?') {
        Some((path, query)) => (path, query),
        None => (target.as_str(), ""),
    };

    match (method.as_str(), path) {
        ("GET", "/ping") => {
            let payload = format!(
                "{{\"ok\":true,\"app\":\"lotus-dm\",\"version\":\"{}\"}}",
                env!("CARGO_PKG_VERSION")
            );
            respond(&mut stream, 200, &payload);
            false
        }
        ("GET", "/add") => {
            let url = query_param(query, "url").unwrap_or_default();
            let filename = query_param(query, "filename");
            let quality = query_param(query, "quality");
            queue_one(&mut stream, tx, url, filename, quality)
        }
        ("POST", "/add") | ("POST", "/") => {
            let parsed: Option<AddBody> = serde_json::from_slice(&body).ok();
            let Some(parsed) = parsed else {
                respond(&mut stream, 400, "{\"ok\":false,\"error\":\"expected a JSON body\"}");
                return false;
            };
            queue_one(&mut stream, tx, parsed.url, parsed.filename, parsed.quality)
        }
        ("GET", "/list") | ("GET", "/status") => {
            let (reply_tx, reply_rx) = mpsc::channel();
            if cmd_tx.send(BridgeCmd::List { reply: reply_tx }).is_ok() {
                match reply_rx.recv_timeout(Duration::from_secs(5)) {
                    Ok(jobs) => {
                        let payload = serde_json::to_string(&jobs).unwrap_or_else(|_| "[]".to_string());
                        respond(&mut stream, 200, &payload);
                    }
                    Err(_) => respond(&mut stream, 503, "{\"ok\":false,\"error\":\"timeout\"}"),
                }
            } else {
                respond(&mut stream, 503, "{\"ok\":false,\"error\":\"window closed\"}");
            }
            false
        }
        ("POST", "/pause") => {
            let id = query_param(query, "id").unwrap_or_default();
            let (reply_tx, reply_rx) = mpsc::channel();
            if cmd_tx.send(BridgeCmd::Pause { id, reply: reply_tx }).is_ok() {
                match reply_rx.recv_timeout(Duration::from_secs(5)) {
                    Ok(ok) => respond(&mut stream, 200, &format!("{{\"ok\":{ok}}}")),
                    Err(_) => respond(&mut stream, 503, "{\"ok\":false,\"error\":\"timeout\"}"),
                }
            } else {
                respond(&mut stream, 503, "{\"ok\":false,\"error\":\"window closed\"}");
            }
            false
        }
        ("POST", "/resume") => {
            let id = query_param(query, "id").unwrap_or_default();
            let (reply_tx, reply_rx) = mpsc::channel();
            if cmd_tx.send(BridgeCmd::Resume { id, reply: reply_tx }).is_ok() {
                match reply_rx.recv_timeout(Duration::from_secs(5)) {
                    Ok(ok) => respond(&mut stream, 200, &format!("{{\"ok\":{ok}}}")),
                    Err(_) => respond(&mut stream, 503, "{\"ok\":false,\"error\":\"timeout\"}"),
                }
            } else {
                respond(&mut stream, 503, "{\"ok\":false,\"error\":\"window closed\"}");
            }
            false
        }
        ("POST", "/cancel") => {
            let id = query_param(query, "id").unwrap_or_default();
            let (reply_tx, reply_rx) = mpsc::channel();
            if cmd_tx.send(BridgeCmd::Cancel { id, reply: reply_tx }).is_ok() {
                match reply_rx.recv_timeout(Duration::from_secs(5)) {
                    Ok(ok) => respond(&mut stream, 200, &format!("{{\"ok\":{ok}}}")),
                    Err(_) => respond(&mut stream, 503, "{\"ok\":false,\"error\":\"timeout\"}"),
                }
            } else {
                respond(&mut stream, 503, "{\"ok\":false,\"error\":\"window closed\"}");
            }
            false
        }
        _ => {
            respond(&mut stream, 404, "{\"ok\":false,\"error\":\"unknown route\"}");
            false
        }
    }
}

fn queue_one(
    stream: &mut TcpStream,
    tx: &Sender<AddRequest>,
    url: String,
    filename: Option<String>,
    quality: Option<String>,
) -> bool {
    let url = url.trim().to_string();
    let lower = url.to_ascii_lowercase();
    let ok = (url.starts_with("http://") || url.starts_with("https://")
        || lower.starts_with("ftp://") || lower.starts_with("magnet:?"))
        && url.len() <= 8192;
    if !ok {
        respond(
            stream,
            400,
            "{\"ok\":false,\"error\":\"needs an http(s), ftp or magnet link\"}",
        );
        return false;
    }
    let filename = filename
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty() && name.len() <= 200);
    let quality = quality
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty() && name.len() <= 20);
    if tx.send(AddRequest { url, filename, quality }).is_err() {
        respond(stream, 503, "{\"ok\":false,\"error\":\"window closed\"}");
        return false;
    }
    respond(stream, 200, "{\"ok\":true}");
    true
}

struct Head {
    head: Vec<u8>,
    rest: Vec<u8>,
}

fn read_headers(stream: &mut TcpStream) -> Option<Head> {
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    loop {
        if let Some(pos) = find(&buf, b"\r\n\r\n") {
            let rest = buf.split_off(pos + 4);
            return Some(Head { head: buf, rest });
        }
        if buf.len() > MAX_HEADER {
            return None;
        }
        match stream.read(&mut chunk) {
            Ok(0) => return Some(Head { head: buf, rest: Vec::new() }),
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(_) => return None,
        }
    }
}

fn read_body(stream: &mut TcpStream, mut rest: Vec<u8>, want: usize) -> Vec<u8> {
    let mut chunk = [0u8; 4096];
    while rest.len() < want {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => rest.extend_from_slice(&chunk[..n]),
            Err(_) => break,
        }
    }
    rest.truncate(want);
    rest
}

fn respond(stream: &mut TcpStream, status: u16, body: &str) {
    let reason = match status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Payload Too Large",
        503 => "Service Unavailable",
        _ => "OK",
    };
    let payload = body.as_bytes();
    let length = if status == 204 { 0 } else { payload.len() };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {length}\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
         Access-Control-Allow-Headers: Content-Type\r\n\
         Access-Control-Allow-Private-Network: true\r\n\
         Access-Control-Max-Age: 600\r\n\
         Cache-Control: no-store\r\n\
         Connection: close\r\n\r\n"
    );
    let _ = stream.write_all(head.as_bytes());
    if status != 204 {
        let _ = stream.write_all(payload);
    }
    let _ = stream.flush();
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn query_param(query: &str, key: &str) -> Option<String> {
    for pair in query.split('&') {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        if name == key {
            return Some(percent_decode(value));
        }
    }
    None
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
                match u8::from_str_radix(hex, 16) {
                    Ok(byte) => {
                        out.push(byte);
                        i += 3;
                    }
                    Err(_) => {
                        out.push(bytes[i]);
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            byte => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}
