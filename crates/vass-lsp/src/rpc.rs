//! JSON-RPC 2.0 message framing for LSP over stdio.
//!
//! Implements the `Content-Length` / `Content-Type` header framing required by
//! the Language Server Protocol, plus helpers for building responses.

use std::io::{self, BufRead, Write};

/// Read one JSON-RPC message from `reader`.
///
/// Returns `Ok(None)` on a clean EOF (client closed the stream).
/// Skips blank lines between messages and tolerates unknown headers.
pub fn read_message(reader: &mut dyn BufRead) -> io::Result<Option<String>> {
  let mut content_length: Option<usize> = None;
  let mut line = String::new();

  loop {
    line.clear();
    let n = reader.read_line(&mut line)?;
    if n == 0 {
      // EOF: a header block was started but never finished; treat as close
      return Ok(None);
    }
    let trimmed = line.trim_end_matches(['\r', '\n']);
    if trimmed.is_empty() {
      // End of headers
      break;
    }
    if let Some((name, value)) = trimmed.split_once(':')
      && name.eq_ignore_ascii_case("content-length")
    {
      content_length = value.trim().parse::<usize>().ok();
      // Content-Type and unknown headers are ignored per spec
    }
  }

  let len = content_length.ok_or_else(|| {
    io::Error::new(
      io::ErrorKind::InvalidData,
      "missing Content-Length header in LSP message",
    )
  })?;

  let mut body = vec![0u8; len];
  reader.read_exact(&mut body)?;
  String::from_utf8(body)
    .map(Some)
    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("invalid UTF-8: {e}")))
}

/// Write one JSON-RPC message to `writer` with proper framing.
pub fn write_message(writer: &mut dyn Write, value: &serde_json::Value) -> io::Result<()> {
  let body = serde_json::to_string(value)
    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("serialize: {e}")))?;
  write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
  writer.write_all(body.as_bytes())?;
  writer.flush()
}

/// Build a successful response for request `id`.
pub fn response_ok(id: serde_json::Value, result: serde_json::Value) -> serde_json::Value {
  serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

/// Build an error response for request `id`.
pub fn response_err(id: serde_json::Value, code: i64, message: &str) -> serde_json::Value {
  serde_json::json!({
      "jsonrpc": "2.0",
      "id": id,
      "error": { "code": code, "message": message }
  })
}

/// Build a server-initiated notification.
pub fn notification(method: &str, params: serde_json::Value) -> serde_json::Value {
  serde_json::json!({ "jsonrpc": "2.0", "method": method, "params": params })
}

/// Standard JSON-RPC error codes used by the server.
pub mod codes {
  /// Method not found
  pub const METHOD_NOT_FOUND: i64 = -32601;
  /// Invalid parameters
  pub const INVALID_PARAMS: i64 = -32602;
  /// Internal error
  pub const INTERNAL_ERROR: i64 = -32603;
  /// Server not initialized
  pub const SERVER_NOT_INITIALIZED: i64 = -32002;
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn roundtrip_message() {
    let mut buf: Vec<u8> = Vec::new();
    let msg = serde_json::json!({ "jsonrpc": "2.0", "id": 1, "result": null });
    write_message(&mut buf, &msg).unwrap();
    assert!(String::from_utf8_lossy(&buf).contains("Content-Length:"));

    let mut reader = std::io::BufReader::new(&buf[..]);
    let read = read_message(&mut reader).unwrap().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&read).unwrap();
    assert_eq!(parsed["id"], 1);
  }

  #[test]
  fn eof_returns_none() {
    let mut reader = std::io::BufReader::new(&b""[..]);
    assert!(read_message(&mut reader).unwrap().is_none());
  }

  #[test]
  fn skips_unknown_headers() {
    let body = r#"{"jsonrpc":"2.0","method":"x"}"#;
    let raw = format!(
      "Content-Type: application/vscode-jsonrpc\r\nContent-Length: {}\r\n\r\n{}",
      body.len(),
      body
    );
    let mut reader = std::io::BufReader::new(raw.as_bytes());
    let read = read_message(&mut reader).unwrap().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&read).unwrap();
    assert_eq!(parsed["method"], "x");
  }

  #[test]
  fn missing_content_length_is_error() {
    let mut reader = std::io::BufReader::new(&b"Content-Length:abc\r\n\r\n"[..]);
    assert!(read_message(&mut reader).is_err());
  }
}
