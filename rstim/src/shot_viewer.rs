use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
#[cfg(not(target_arch = "wasm32"))]
use std::process::Command;
use std::time::Duration;

const INDEX_HTML: &[u8] = include_bytes!("../assets/shot-viewer/index.html");
const APP_JS: &[u8] = include_bytes!("../assets/shot-viewer/app.js");
const VIEWER_CSS: &[u8] = include_bytes!("../assets/shot-viewer/shot-viewer.css");
const WASM: &[u8] = include_bytes!("../assets/shot-viewer/pkg/rstim_shot_web_bg.wasm");
const MAX_REQUEST_HEADER_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, Copy)]
pub struct ShotViewerOptions {
    pub port: u16,
    pub open_browser: bool,
    pub serve_once: bool,
}

impl Default for ShotViewerOptions {
    fn default() -> Self {
        Self {
            port: 0,
            open_browser: true,
            serve_once: false,
        }
    }
}

pub fn serve(options: ShotViewerOptions) -> Result<(), String> {
    let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, options.port))
        .map_err(|error| format!("failed to bind the local shot viewer: {error}"))?;
    let address = listener
        .local_addr()
        .map_err(|error| format!("failed to inspect the local shot viewer address: {error}"))?;
    let url = format!("http://127.0.0.1:{}/", address.port());
    println!("rstim shot viewer: {url}");
    println!("Press Ctrl-C to stop. Circuit files remain inside your browser.");

    if options.open_browser {
        open_browser(&url);
    }

    for incoming in listener.incoming() {
        let mut stream =
            incoming.map_err(|error| format!("shot viewer connection failed: {error}"))?;
        if let Err(error) = handle_connection(&mut stream, address.port()) {
            eprintln!("Ignored invalid local shot viewer request: {error}");
        }
        if options.serve_once {
            break;
        }
    }
    Ok(())
}

fn handle_connection(stream: &mut TcpStream, port: u16) -> Result<(), String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| format!("failed to set shot viewer read timeout: {error}"))?;
    let request = read_request(stream)?;
    let mut lines = request.split("\r\n");
    let request_line = lines.next().unwrap_or_default();
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts.next().unwrap_or_default();
    let raw_target = request_parts.next().unwrap_or_default();
    let version = request_parts.next().unwrap_or_default();
    if request_parts.next().is_some() || !matches!(version, "HTTP/1.0" | "HTTP/1.1") {
        return write_response(
            stream,
            400,
            "Bad Request",
            "text/plain",
            b"bad request",
            false,
        );
    }
    let host = lines.find_map(|line| {
        line.split_once(':')
            .filter(|(name, _)| name.eq_ignore_ascii_case("host"))
            .map(|(_, value)| value.trim())
    });
    if !valid_host(host, port) {
        return write_response(stream, 403, "Forbidden", "text/plain", b"forbidden", false);
    }
    if !matches!(method, "GET" | "HEAD") {
        return write_response(
            stream,
            405,
            "Method Not Allowed",
            "text/plain",
            b"method not allowed",
            method == "HEAD",
        );
    }
    let target = raw_target.split('?').next().unwrap_or(raw_target);
    let (content_type, body) = match target {
        "/" | "/index.html" => ("text/html; charset=utf-8", INDEX_HTML),
        "/app.js" => ("text/javascript; charset=utf-8", APP_JS),
        "/shot-viewer.css" => ("text/css; charset=utf-8", VIEWER_CSS),
        "/pkg/rstim_shot_web_bg.wasm" => ("application/wasm", WASM),
        "/favicon.ico" => {
            return write_response(stream, 204, "No Content", "image/x-icon", b"", true);
        }
        _ => {
            return write_response(
                stream,
                404,
                "Not Found",
                "text/plain",
                b"not found",
                method == "HEAD",
            );
        }
    };
    write_response(stream, 200, "OK", content_type, body, method == "HEAD")
}

fn read_request(stream: &mut TcpStream) -> Result<String, String> {
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 1024];
    while bytes.len() < MAX_REQUEST_HEADER_BYTES {
        let read = stream
            .read(&mut chunk)
            .map_err(|error| format!("failed to read shot viewer request: {error}"))?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..read]);
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            return String::from_utf8(bytes)
                .map_err(|_| "shot viewer request headers are not valid UTF-8".to_string());
        }
    }
    Err("shot viewer request headers are incomplete or too large".to_string())
}

fn valid_host(host: Option<&str>, port: u16) -> bool {
    let Some(host) = host else {
        return false;
    };
    host.eq_ignore_ascii_case(&format!("127.0.0.1:{port}"))
        || host.eq_ignore_ascii_case(&format!("localhost:{port}"))
}

fn write_response(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    content_type: &str,
    body: &[u8],
    head_only: bool,
) -> Result<(), String> {
    let headers = format!(
        "HTTP/1.1 {status} {reason}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         Cache-Control: no-store\r\n\
         X-Content-Type-Options: nosniff\r\n\
         Referrer-Policy: no-referrer\r\n\
         Content-Security-Policy: default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' blob: data:; connect-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'\r\n\
         Connection: close\r\n\r\n",
        body.len()
    );
    stream
        .write_all(headers.as_bytes())
        .and_then(|()| {
            if head_only {
                Ok(())
            } else {
                stream.write_all(body)
            }
        })
        .map_err(|error| format!("failed to write shot viewer response: {error}"))
}

#[cfg(not(target_arch = "wasm32"))]
fn open_browser(url: &str) {
    #[cfg(target_os = "macos")]
    let result = Command::new("open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let result = Command::new("cmd").args(["/C", "start", "", url]).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let result = Command::new("xdg-open").arg(url).spawn();
    if let Err(error) = result {
        eprintln!("Could not open a browser automatically ({error}). Open {url} manually.");
    }
}

#[cfg(target_arch = "wasm32")]
fn open_browser(_url: &str) {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpStream;
    use std::thread;

    fn round_trip(request: &str) -> Vec<u8> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let request = request.replace("{port}", &port.to_string());
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            handle_connection(&mut stream, port).unwrap();
        });
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        stream.write_all(request.as_bytes()).unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).unwrap();
        server.join().unwrap();
        response
    }

    #[test]
    fn serves_version_matched_wasm_with_strict_headers() {
        let response = round_trip(
            "GET /pkg/rstim_shot_web_bg.wasm HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n",
        );
        assert!(response.starts_with(b"HTTP/1.1 200 OK\r\n"));
        let mime = b"Content-Type: application/wasm";
        assert!(response.windows(mime.len()).any(|window| window == mime));
        let wasm_csp = b"script-src 'self' 'wasm-unsafe-eval'";
        assert!(
            response
                .windows(wasm_csp.len())
                .any(|window| window == wasm_csp)
        );
        assert!(response.windows(4).any(|window| window == b"\0asm"));
    }

    #[test]
    fn rejects_non_local_host_headers() {
        let response = round_trip("GET / HTTP/1.1\r\nHost: attacker.example\r\n\r\n");
        assert!(response.starts_with(b"HTTP/1.1 403 Forbidden\r\n"));
    }

    #[test]
    fn rejects_path_traversal_instead_of_touching_the_filesystem() {
        let response = round_trip("GET /../Cargo.toml HTTP/1.1\r\nHost: localhost:{port}\r\n\r\n");
        assert!(response.starts_with(b"HTTP/1.1 404 Not Found\r\n"));
    }
}
