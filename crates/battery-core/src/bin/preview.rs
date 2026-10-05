//! Local development bridge. Synthetic hardware only; never shipped as a production network service.
use battery_deadline_core::{
    Error, Result,
    service::{Command, Service},
};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    time::Duration,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("BatteryDeadline preview: {error}");
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let directory = std::env::temp_dir().join("battery-deadline-browser-preview");
    let service = Service::spawn(directory, true);
    let listener = TcpListener::bind("127.0.0.1:1421")?;
    println!(
        "Synthetic preview bridge: http://127.0.0.1:1421\nOpen Vite at http://127.0.0.1:1420/?simulate\nNo Windows settings are changed."
    );
    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let result = handle(&mut stream, &service);
                if let Err(error) = result {
                    let _ = respond(
                        &mut stream,
                        400,
                        &serde_json::json!({"error":error.to_string()}),
                    );
                }
            }
            Err(error) => eprintln!("Preview connection: {error}"),
        }
    }
    Ok(())
}
fn handle(stream: &mut TcpStream, service: &Service) -> Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    stream.set_write_timeout(Some(Duration::from_secs(3)))?;
    let mut bytes = Vec::new();
    let header_end;
    loop {
        let mut part = [0_u8; 4096];
        let n = stream.read(&mut part)?;
        if n == 0 {
            return Err(Error::InvalidInput("Empty request.".into()));
        }
        bytes.extend_from_slice(&part[..n]);
        if bytes.len() > 65_536 {
            return Err(Error::InvalidInput("Request is too large.".into()));
        }
        if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
            header_end = end + 4;
            break;
        }
    }
    let header = String::from_utf8_lossy(&bytes[..header_end]);
    let mut lines = header.lines();
    if lines.next() != Some("POST /__simulation HTTP/1.1") {
        return Err(Error::InvalidInput(
            "Only local simulation commands are accepted.".into(),
        ));
    }
    let mut length = None;
    let mut origin = false;
    for line in lines {
        if let Some((key, value)) = line.split_once(':') {
            if key.eq_ignore_ascii_case("content-length") {
                length = value.trim().parse::<usize>().ok();
            }
            if key.eq_ignore_ascii_case("origin") {
                origin = ["http://127.0.0.1:1420", "http://localhost:1420"].contains(&value.trim());
            }
        }
    }
    if !origin {
        return Err(Error::InvalidInput(
            "Open the local Vite preview to use simulation commands.".into(),
        ));
    }
    let length = length
        .filter(|l| *l <= 32_768)
        .ok_or_else(|| Error::InvalidInput("Invalid request length.".into()))?;
    while bytes.len() < header_end + length {
        let mut part = [0_u8; 4096];
        let n = stream.read(&mut part)?;
        if n == 0 {
            return Err(Error::InvalidInput("Incomplete request.".into()));
        }
        bytes.extend_from_slice(&part[..n]);
    }
    let command: Command = serde_json::from_slice(&bytes[header_end..header_end + length])?;
    match service.request(command) {
        Ok(value) => respond(stream, 200, &value),
        Err(error) => respond(stream, 400, &serde_json::json!({"error":error.to_string()})),
    }
}
fn respond(stream: &mut TcpStream, status: u16, value: &serde_json::Value) -> Result<()> {
    let body = serde_json::to_vec(value)?;
    write!(
        stream,
        "HTTP/1.1 {status} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        if status == 200 { "OK" } else { "Bad Request" },
        body.len()
    )?;
    stream.write_all(&body)?;
    Ok(())
}
