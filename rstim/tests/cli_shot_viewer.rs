use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn shot_viewer_help_documents_loopback_controls() {
    let output = Command::new(env!("CARGO_BIN_EXE_rstim"))
        .args(["shot_viewer", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("--port"));
    assert!(stdout.contains("--no_open"));
    assert!(!stdout.contains("--serve-once"));
}

#[test]
fn shot_viewer_serves_one_loopback_request_through_cli_dispatch() {
    let reservation = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = reservation.local_addr().unwrap().port();
    drop(reservation);

    let mut child = Command::new(env!("CARGO_BIN_EXE_rstim"))
        .args([
            "shot_viewer",
            "--port",
            &port.to_string(),
            "--no_open",
            "--serve-once",
        ])
        .spawn()
        .unwrap();

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut stream = loop {
        match TcpStream::connect((Ipv4Addr::LOCALHOST, port)) {
            Ok(stream) => break stream,
            Err(_) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => {
                child.kill().unwrap();
                panic!("shot viewer did not bind to loopback: {error}");
            }
        }
    };
    write!(
        stream,
        "GET /index.html HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"
    )
    .unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();
    assert!(response.starts_with(b"HTTP/1.1 200 OK\r\n"));
    let app_marker = b"id=\"shot-viewer\"";
    assert!(response
        .windows(app_marker.len())
        .any(|window| window == app_marker));

    let status = child.wait().unwrap();
    assert!(status.success());
}
