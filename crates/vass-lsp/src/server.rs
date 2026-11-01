//! The LSP server: state, dispatch loop and all request/notification
//! handlers. Transport-agnostic — I/O is passed in as reader/writer.

use std::collections::HashMap;
use std::io::{BufRead, BufWriter, Write};
use std::path::PathBuf;

use serde_json::{Value, json};

use crate::analysis::{self, Index};
use crate::completion;
use crate::diag;
use crate::format;
use crate::rpc;

/// Convert an LSP URI into a filesystem path (best effort).
pub fn uri_to_path(uri: &str) -> Option<PathBuf> {
  let rest = uri.strip_prefix("file://")?;
  // strip authority (usually empty or localhost)
  let rest = rest.strip_prefix("localhost").unwrap_or(rest);
  Some(PathBuf::from(percent_decode(rest)))
}

/// Percent-decode a URI path component (UTF-8, lossy on bad escapes)
fn percent_decode(s: &str) -> String {
  let b = s.as_bytes();
  let mut out = Vec::with_capacity(b.len());
  let mut i = 0;
  while i < b.len() {
    if b[i] == b'%' && i + 2 < b.len() {
      let hex = std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("");
      if let Ok(v) = u8::from_str_radix(hex, 16) {
        out.push(v);
        i += 3;
        continue;
      }
    }
    out.push(b[i]);
    i += 1;
  }
  String::from_utf8_lossy(&out).into_owned()
}

struct Document {
  text: String,
  _version: Option<i64>,
}

/// The VASS language server.
pub struct Server {
  documents: HashMap<String, Document>,
  initialized: bool,
  shutdown_requested: bool,
}

impl Default for Server {
  fn default() -> Self {
    Self::new()
  }
}

impl Server {
  /// Create a new server instance
  pub fn new() -> Self {
    Self {
      documents: HashMap::new(),
      initialized: false,
      shutdown_requested: false,
    }
  }

  /// Run the request loop until `exit`. Returns the process exit code:
  /// 0 after a proper `shutdown` + `exit`, 1 otherwise.
  pub fn run(&mut self, input: &mut dyn BufRead, output: &mut dyn Write) -> i32 {
    loop {
      let raw = match rpc::read_message(input) {
        Ok(Some(m)) => m,
        Ok(None) => return if self.shutdown_requested { 0 } else { 1 },
        Err(e) => {
          tracing::error!("read error: {e}");
          return 1;
        }
      };
      let msg: Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
          tracing::error!("malformed message: {e}");
          continue;
        }
      };
      match self.handle(&msg, output) {
        Ok(Flow::Continue) => {}
        Ok(Flow::Exit(code)) => return code,
        Err(e) => {
          tracing::error!("handler error: {e}");
        }
      }
    }
  }

  /// Handle one incoming message; public for tests.
  pub fn handle(&mut self, msg: &Value, output: &mut dyn Write) -> std::io::Result<Flow> {
    let method = msg["method"].as_str().unwrap_or("");
    let has_id = msg.get("id").is_some_and(|v| !v.is_null());

    if has_id {
      let id = msg["id"].clone();
      if !self.initialized && method != "initialize" {
        let resp = rpc::response_err(
          id,
          rpc::codes::SERVER_NOT_INITIALIZED,
          "server not initialized",
        );
        return rpc::write_message(output, &resp).map(|_| Flow::Continue);
      }
      let result = self.handle_request(method, &msg["params"]);
      let resp = match result {
        Ok(v) => rpc::response_ok(id, v),
        Err((code, message)) => rpc::response_err(id, code, &message),
      };
      rpc::write_message(output, &resp)?;
      if method == "shutdown" {
        self.shutdown_requested = true;
      }
      return Ok(Flow::Continue);
    }

    // notifications
    match method {
      "initialize" => {} // not allowed as notification; ignore
      "initialized" => self.initialized = true,
      "exit" => {
        return Ok(Flow::Exit(if self.shutdown_requested { 0 } else { 1 }));
      }
      "textDocument/didOpen" => self.did_open(&msg["params"], output)?,
      "textDocument/didChange" => self.did_change(&msg["params"], output)?,
      "textDocument/didClose" => self.did_close(&msg["params"], output)?,
      "textDocument/didSave" => {}
      m if m.starts_with("$/") => {}
      _ => {}
    }
    Ok(Flow::Continue)
  }

  fn handle_request(&mut self, method: &str, params: &Value) -> Result<Value, (i64, String)> {
    match method {
      "initialize" => Ok(self.initialize()),
      "shutdown" => Ok(Value::Null),
      "textDocument/completion" => {
        let uri = params["textDocument"]["uri"].as_str().unwrap_or("");
        let line = params["position"]["line"].as_u64().unwrap_or(0) as u32;
        let character = params["position"]["character"].as_u64().unwrap_or(0) as u32;
        let Some(doc) = self.documents.get(uri) else {
          return Ok(Value::Null);
        };
        let index = analysis::analyze(&doc.text);
        let items = completion::completions(&index, &doc.text, line, character);
        Ok(json!({ "isIncomplete": false, "items": items }))
      }
      "textDocument/hover" => {
        let uri = params["textDocument"]["uri"].as_str().unwrap_or("");
        let line = params["position"]["line"].as_u64().unwrap_or(0) as u32;
        let character = params["position"]["character"].as_u64().unwrap_or(0) as u32;
        let Some(doc) = self.documents.get(uri) else {
          return Ok(Value::Null);
        };
        let index = analysis::analyze(&doc.text);
        Ok(completion::hover(&index, &doc.text, line, character).unwrap_or(Value::Null))
      }
      "textDocument/definition" => {
        let uri = params["textDocument"]["uri"].as_str().unwrap_or("");
        let line = params["position"]["line"].as_u64().unwrap_or(0) as u32;
        let character = params["position"]["character"].as_u64().unwrap_or(0) as u32;
        Ok(self.definition(uri, line, character))
      }
      "textDocument/documentSymbol" => {
        let uri = params["textDocument"]["uri"].as_str().unwrap_or("");
        let Some(doc) = self.documents.get(uri) else {
          return Ok(Value::Null);
        };
        let index = analysis::analyze(&doc.text);
        Ok(serde_json::to_value(index.doc_symbols).unwrap_or(Value::Null))
      }
      "textDocument/formatting" => {
        let uri = params["textDocument"]["uri"].as_str().unwrap_or("");
        let Some(doc) = self.documents.get(uri) else {
          return Ok(Value::Null);
        };
        let braces = !uri.ends_with(".vass");
        let formatted = format::format_source(&doc.text, braces);
        if formatted == doc.text {
          return Ok(Value::Null);
        }
        let last_line = doc.text.lines().count() as u32;
        let last_len = doc.text.lines().last().map(|l| l.len()).unwrap_or(0) as u32;
        Ok(json!([{
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": last_line, "character": last_len },
            },
            "newText": formatted,
        }]))
      }
      "textDocument/codeAction" => {
        let uri = params["textDocument"]["uri"].as_str().unwrap_or("");
        Ok(self.code_actions(uri, &params["context"]))
      }
      _ => Err((
        rpc::codes::METHOD_NOT_FOUND,
        format!("method not found: {method}"),
      )),
    }
  }

  /// Turn vass diagnostics carrying `data.fix` into LSP quick fixes
  /// (editor lightbulb actions). The fix shape comes from `diag.rs`:
  /// `{ title, replace }` replaces the diagnostic's range, or
  /// `{ title, insert: { line, character, text } }` inserts at a spot.
  fn code_actions(&self, uri: &str, context: &Value) -> Value {
    let Some(diags) = context["diagnostics"].as_array() else {
      return Value::Array(Vec::new());
    };
    let actions: Vec<Value> = diags
      .iter()
      .filter(|d| d["source"].as_str() == Some("vass"))
      .filter_map(|d| {
        let fix = &d["data"]["fix"];
        if fix.is_null() {
          return None;
        }
        let title = fix["title"].as_str()?;
        let (range, new_text) = if let Some(text) = fix["replace"].as_str() {
          (d["range"].clone(), json!(text))
        } else {
          let ins = fix.get("insert")?;
          let line = ins["line"].as_u64().unwrap_or(0) as u32;
          let ch = ins["character"].as_u64().unwrap_or(0) as u32;
          let text = ins["text"].as_str()?;
          (
            json!({
                "start": { "line": line, "character": ch },
                "end": { "line": line, "character": ch },
            }),
            json!(text),
          )
        };
        Some(json!({
            "title": title,
            "kind": "quickfix",
            "diagnostics": [d],
            "edit": { "changes": { uri: [{ "range": range, "newText": new_text }] } },
        }))
      })
      .collect();
    Value::Array(actions)
  }

  fn initialize(&mut self) -> Value {
    self.initialized = true;
    json!({
        "capabilities": {
            "positionEncoding": "utf-16",
            "textDocumentSync": {
                "openClose": true,
                "change": 1, // Full
            },
            "completionProvider": {
                "triggerCharacters": ["$", ".", "@", "-"],
                "resolveProvider": false,
            },
            "hoverProvider": true,
            "definitionProvider": true,
            "documentSymbolProvider": true,
            "documentFormattingProvider": true,
            "codeActionProvider": true,
        },
        "serverInfo": {
            "name": "vass-lsp-server",
            "version": env!("CARGO_PKG_VERSION"),
        }
    })
  }

  fn doc_syntax_is_indented(uri: &str) -> bool {
    uri.ends_with(".vass")
  }

  fn compute_diagnostics(&self, uri: &str) -> Vec<Value> {
    let Some(doc) = self.documents.get(uri) else {
      return Vec::new();
    };
    let path = uri_to_path(uri);
    diag::compute(&doc.text, path.as_deref())
  }

  fn publish_diagnostics(&self, uri: &str, output: &mut dyn Write) -> std::io::Result<()> {
    let diagnostics = self.compute_diagnostics(uri);
    let msg = rpc::notification(
      "textDocument/publishDiagnostics",
      json!({ "uri": uri, "diagnostics": diagnostics }),
    );
    rpc::write_message(output, &msg)
  }

  fn did_open(&mut self, params: &Value, output: &mut dyn Write) -> std::io::Result<()> {
    let td = &params["textDocument"];
    let uri = td["uri"].as_str().unwrap_or("").to_string();
    let text = td["text"].as_str().unwrap_or("").to_string();
    let version = td["version"].as_i64();
    self.documents.insert(
      uri.clone(),
      Document {
        text,
        _version: version,
      },
    );
    self.publish_diagnostics(&uri, output)
  }

  fn did_change(&mut self, params: &Value, output: &mut dyn Write) -> std::io::Result<()> {
    let uri = params["textDocument"]["uri"]
      .as_str()
      .unwrap_or("")
      .to_string();
    let version = params["textDocument"]["version"].as_i64();
    // full sync: take the last content change's text
    let Some(changes) = params["contentChanges"].as_array() else {
      return Ok(());
    };
    let Some(last) = changes.last() else {
      return Ok(());
    };
    let Some(text) = last["text"].as_str() else {
      return Ok(());
    };
    if let Some(doc) = self.documents.get_mut(&uri) {
      doc.text = text.to_string();
      doc._version = version;
    }
    self.publish_diagnostics(&uri, output)
  }

  fn did_close(&mut self, params: &Value, output: &mut dyn Write) -> std::io::Result<()> {
    let uri = params["textDocument"]["uri"]
      .as_str()
      .unwrap_or("")
      .to_string();
    self.documents.remove(&uri);
    let msg = rpc::notification(
      "textDocument/publishDiagnostics",
      json!({ "uri": uri, "diagnostics": [] }),
    );
    rpc::write_message(output, &msg)
  }

  fn definition(&self, uri: &str, line: u32, character: u32) -> Value {
    let Some(doc) = self.documents.get(uri) else {
      return Value::Null;
    };
    let index: Index = analysis::analyze(&doc.text);
    let Some((word, _start, _end)) = analysis::word_at(&doc.text, line, character) else {
      return Value::Null;
    };
    let target = if let Some(name) = word.strip_prefix('$') {
      index
        .symbols
        .iter()
        .find(|s| s.kind == analysis::SymbolKind::Variable && s.name == name)
    } else {
      index.symbols.iter().find(|s| {
        s.name == word
          && (s.kind == analysis::SymbolKind::Mixin || s.kind == analysis::SymbolKind::Function)
      })
    };
    match target {
      Some(s) => {
        let len = word.len() as u32;
        json!({
            "uri": uri,
            "range": {
                "start": { "line": s.line, "character": s.col },
                "end": { "line": s.line, "character": s.col + len },
            },
        })
      }
      None => Value::Null,
    }
  }
}

/// Outcome of handling one message
pub enum Flow {
  /// Keep serving
  Continue,
  /// Stop with the given exit code
  Exit(i32),
}

/// Run the LSP server over stdio with buffered output.
///
/// Creates a `BufWriter` around stdout so each `Content-Length` frame is
/// written efficiently, then runs the server request loop. The writer is
/// dropped (and thus flushed) before this function returns, so no buffered
/// LSP output is lost on exit.
///
/// Returns the process exit code: `0` after a proper `shutdown` + `exit`,
/// `1` otherwise.
pub fn run_stdio() -> i32 {
  let stdin = std::io::stdin();
  let stdout = std::io::stdout();
  let mut reader = stdin.lock();
  let mut writer = BufWriter::new(stdout.lock());
  let mut server = Server::new();
  let code = server.run(&mut reader, &mut writer);
  // `writer` is dropped here, flushing any buffered output before returning.
  code
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::io::BufReader;

  fn exchange(server: &mut Server, input: &str) -> (Vec<Value>, i32) {
    let mut out: Vec<u8> = Vec::new();
    let code = server.run(&mut BufReader::new(input.as_bytes()), &mut out);
    let text = String::from_utf8(out).unwrap();
    let msgs: Vec<Value> = text
      .split("Content-Length:")
      .skip(1)
      .filter_map(|part| {
        let body = part.split("\r\n\r\n").nth(1)?;
        serde_json::from_str(body).ok()
      })
      .collect();
    (msgs, code)
  }

  fn request(id: u64, method: &str, params: Value) -> String {
    rpc::write_message(
      &mut Vec::new(),
      &json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }),
    )
    .unwrap();
    // rebuild framing manually for the raw string test helper
    let msg = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
    let body = serde_json::to_string(&msg).unwrap();
    format!("Content-Length: {}\r\n\r\n{}", body.len(), body)
  }

  fn notification(method: &str, params: Value) -> String {
    let msg = json!({ "jsonrpc": "2.0", "method": method, "params": params });
    let body = serde_json::to_string(&msg).unwrap();
    format!("Content-Length: {}\r\n\r\n{}", body.len(), body)
  }

  #[test]
  fn full_session_vcss() {
    let mut server = Server::new();
    let uri = "file:///tmp/lsp_test.vcss";
    let mut input = String::new();
    input.push_str(&request(1, "initialize", json!({ "capabilities": {} })));
    input.push_str(&notification("initialized", json!({})));
    input.push_str(&notification(
      "textDocument/didOpen",
      json!({ "textDocument": {
          "uri": uri, "languageId": "vass", "version": 1,
          "text": "$accent: #e63946;\n.a { color: $accent; }\n"
      }}),
    ));
    input.push_str(&request(
      2,
      "textDocument/completion",
      json!({
          "textDocument": { "uri": uri },
          "position": { "line": 1, "character": 18 }
      }),
    ));
    input.push_str(&request(
      3,
      "textDocument/hover",
      json!({
          "textDocument": { "uri": uri },
          "position": { "line": 1, "character": 17 }
      }),
    ));
    input.push_str(&request(
      4,
      "textDocument/definition",
      json!({
          "textDocument": { "uri": uri },
          "position": { "line": 1, "character": 17 }
      }),
    ));
    input.push_str(&request(
      5,
      "textDocument/documentSymbol",
      json!({
          "textDocument": { "uri": uri }
      }),
    ));
    input.push_str(&request(
      6,
      "textDocument/formatting",
      json!({
          "textDocument": { "uri": uri },
          "options": { "tabSize": 2, "insertSpaces": true }
      }),
    ));
    input.push_str(&request(7, "shutdown", json!(null)));
    input.push_str(&notification("exit", json!(null)));

    let (msgs, code) = exchange(&mut server, &input);
    assert_eq!(code, 0);

    let by_id = |id: u64| msgs.iter().find(|m| m["id"] == id).cloned().unwrap();

    assert_eq!(by_id(1)["result"]["capabilities"]["hoverProvider"], true);
    // didOpen → publishDiagnostics with no errors
    let diags = msgs
      .iter()
      .find(|m| m["method"] == "textDocument/publishDiagnostics")
      .unwrap();
    assert_eq!(diags["params"]["diagnostics"].as_array().unwrap().len(), 0);
    // completion includes local variable
    let resp2 = by_id(2);
    let items = resp2["result"]["items"].as_array().unwrap();
    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
    assert!(labels.contains(&"$accent"), "{labels:?}");
    // hover on $accent
    let hover = by_id(3)["result"].clone();
    assert!(
      hover["contents"]["value"]
        .as_str()
        .unwrap()
        .contains("$accent: #e63946")
    );
    // definition points at line 0
    let def = by_id(4)["result"].clone();
    assert_eq!(def["range"]["start"]["line"], 0);
    // document symbols contain .a
    let resp5 = by_id(5);
    let syms = resp5["result"].as_array().unwrap();
    assert!(syms.iter().any(|s| s["name"] == ".a"));
    // formatting produces text edit
    let resp6 = by_id(6);
    let edits = resp6["result"].as_array().unwrap();
    assert!(edits[0]["newText"].as_str().unwrap().contains(".a {"));
  }

  #[test]
  fn diagnostics_flow_and_didchange() {
    let mut server = Server::new();
    let uri = "file:///tmp/diag.vcss";
    let mut input = String::new();
    input.push_str(&request(1, "initialize", json!({})));
    input.push_str(&notification("initialized", json!({})));
    input.push_str(&notification(
      "textDocument/didOpen",
      json!({ "textDocument": {
          "uri": uri, "languageId": "vass", "version": 1,
          "text": ".a { color: $missing; }\n"
      }}),
    ));
    input.push_str(&notification(
      "textDocument/didChange",
      json!({ "textDocument": { "uri": uri, "version": 2 },
                    "contentChanges": [ { "text": ".a { color: red; }\n" } ] }),
    ));
    input.push_str(&notification(
      "textDocument/didClose",
      json!({ "textDocument": { "uri": uri } }),
    ));
    input.push_str(&request(2, "shutdown", json!(null)));
    input.push_str(&notification("exit", json!(null)));

    let (msgs, code) = exchange(&mut server, &input);
    assert_eq!(code, 0);
    let pubs: Vec<&Value> = msgs
      .iter()
      .filter(|m| m["method"] == "textDocument/publishDiagnostics")
      .collect();
    assert_eq!(pubs.len(), 3);
    assert_eq!(
      pubs[0]["params"]["diagnostics"].as_array().unwrap().len(),
      1,
      "error expected"
    );
    assert_eq!(
      pubs[1]["params"]["diagnostics"].as_array().unwrap().len(),
      0,
      "fixed after change"
    );
    assert_eq!(
      pubs[2]["params"]["diagnostics"].as_array().unwrap().len(),
      0,
      "cleared on close"
    );
  }

  #[test]
  fn request_before_initialize_rejected() {
    let mut server = Server::new();
    let mut input = request(
      1,
      "textDocument/hover",
      json!({
          "textDocument": { "uri": "file:///x.vcss" },
          "position": { "line": 0, "character": 0 }
      }),
    );
    input.push_str(&notification("exit", json!(null)));
    let (msgs, code) = exchange(&mut server, &input);
    assert_eq!(code, 1, "exit without shutdown must exit 1");
    assert_eq!(msgs[0]["error"]["code"], -32002);
  }

  #[test]
  fn unknown_request_returns_method_not_found() {
    let mut server = Server::new();
    let mut input = request(1, "initialize", json!({}));
    input.push_str(&request(2, "textDocument/fakeMethod", json!({})));
    input.push_str(&notification("exit", json!(null)));
    let (msgs, _) = exchange(&mut server, &input);
    assert_eq!(msgs[1]["error"]["code"], -32601);
  }

  #[test]
  fn indented_document_end_to_end() {
    let mut server = Server::new();
    let uri = "file:///tmp/indent.vass";
    let mut input = String::new();
    input.push_str(&request(1, "initialize", json!({})));
    input.push_str(&notification("initialized", json!({})));
    input.push_str(&notification(
      "textDocument/didOpen",
      json!({ "textDocument": {
          "uri": uri, "languageId": "vass", "version": 1,
          "text": "$c: red\n.a\n\tcolor: $c\n"
      }}),
    ));
    input.push_str(&request(
      2,
      "textDocument/hover",
      json!({
          "textDocument": { "uri": uri },
          "position": { "line": 2, "character": 12 }
      }),
    ));
    input.push_str(&request(
      3,
      "textDocument/definition",
      json!({
          "textDocument": { "uri": uri },
          "position": { "line": 2, "character": 12 }
      }),
    ));
    input.push_str(&request(
      4,
      "textDocument/formatting",
      json!({
          "textDocument": { "uri": uri }
      }),
    ));
    input.push_str(&request(5, "shutdown", json!(null)));
    input.push_str(&notification("exit", json!(null)));

    let (msgs, code) = exchange(&mut server, &input);
    assert_eq!(code, 0);
    let by_id = |id: u64| msgs.iter().find(|m| m["id"] == id).cloned().unwrap();
    assert!(
      by_id(2)["result"]["contents"]["value"]
        .as_str()
        .unwrap()
        .contains("$c: red")
    );
    assert_eq!(by_id(3)["result"]["range"]["start"]["line"], 0);
    // tab-indented `  color: $c` stays as-is (already 2-space here)
    let resp4 = by_id(4);
    let edits = resp4["result"].as_array().unwrap();
    assert!(edits[0]["newText"].as_str().unwrap().contains("color: $c"));
  }

  #[test]
  fn code_action_quick_fix_for_missing_use() {
    // E0011 (no @use) publishes a diagnostic with data.fix; echoing it
    // back through textDocument/codeAction yields an insert edit.
    let mut server = Server::new();
    let uri = "file:///tmp/ns.vcss";
    let mut input = String::new();
    input.push_str(&request(1, "initialize", json!({})));
    input.push_str(&notification("initialized", json!({})));
    input.push_str(&notification(
      "textDocument/didOpen",
      json!({ "textDocument": {
          "uri": uri, "languageId": "vass", "version": 1,
          "text": ".a { x: base.$v; }\n"
      }}),
    ));
    input.push_str(&request(2, "shutdown", json!(null)));
    input.push_str(&notification("exit", json!(null)));

    let (msgs, code) = exchange(&mut server, &input);
    assert_eq!(code, 0);
    let pub_msg = msgs
      .iter()
      .find(|m| m["method"] == "textDocument/publishDiagnostics")
      .expect("diagnostics published");
    let diag = pub_msg["params"]["diagnostics"].as_array().unwrap()[0].clone();
    assert_eq!(diag["code"], "E0011");
    assert_eq!(diag["data"]["fix"]["title"], "Add @use 'base';");

    // codeAction round-trip with the published diagnostic
    let mut server2 = Server::new();
    let mut input2 = String::new();
    input2.push_str(&request(1, "initialize", json!({})));
    input2.push_str(&notification("initialized", json!({})));
    input2.push_str(&request(
      2,
      "textDocument/codeAction",
      json!({
          "textDocument": { "uri": uri },
          "context": { "diagnostics": [diag] },
      }),
    ));
    input2.push_str(&request(3, "shutdown", json!(null)));
    input2.push_str(&notification("exit", json!(null)));
    let (msgs2, _) = exchange(&mut server2, &input2);
    let actions = msgs2
      .iter()
      .find(|m| m["id"] == 2)
      .expect("codeAction response")["result"]
      .as_array()
      .expect("actions array");
    assert_eq!(actions.len(), 1, "{actions:?}");
    assert_eq!(actions[0]["kind"], "quickfix");
    assert_eq!(actions[0]["title"], "Add @use 'base';");
    let edit = &actions[0]["edit"]["changes"][uri][0];
    assert_eq!(edit["newText"], "@use 'base';\n");
    assert_eq!(edit["range"]["start"]["line"], 0);
  }

  #[test]
  fn uri_to_path_decodes() {
    assert_eq!(
      uri_to_path("file:///home/user/my%20dir/a.vcss"),
      Some(PathBuf::from("/home/user/my dir/a.vcss"))
    );
    assert_eq!(uri_to_path("untitled:Untitled-1"), None);
  }
}
