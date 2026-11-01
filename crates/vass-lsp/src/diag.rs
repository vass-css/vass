//! Diagnostics: run the parse + evaluate pipeline and convert errors into
//! LSP diagnostics with 0-based positions.

use serde_json::Value;

use vass_core::ast::input::Syntax;
use vass_core::error::FixKind;

/// LSP diagnostic severity levels
pub const SEVERITY_ERROR: u32 = 1;
/// LSP diagnostic severity levels
pub const SEVERITY_WARNING: u32 = 2;

/// Convert VASS 1-based `Position` parts to a 0-based LSP position array
fn lsp_pos(line: u32, column: u32) -> [u32; 2] {
  [line.saturating_sub(1), column.saturating_sub(1)]
}

fn span_to_range(span: &vass_core::token::Span) -> Value {
  let s = lsp_pos(span.start.line, span.start.column);
  let e = lsp_pos(span.end.line, span.end.column.max(span.start.column + 1));
  serde_json::json!({
      "start": { "line": s[0], "character": s[1] },
      "end": { "line": e[0], "character": e[1] },
  })
}

/// Parse error text like `file:line:col` may be embedded in messages; strip it
fn clean_message(msg: &str) -> String {
  msg.lines().next().unwrap_or(msg).to_string()
}

/// Filesystem path -> `file://` URI (best effort, percent-encodes spaces)
pub fn path_to_uri(path: &str) -> String {
  let norm = path.replace('\\', "/");
  let encoded = norm.replace(' ', "%20");
  if encoded.starts_with("file://") {
    return encoded;
  }
  let absolute = encoded.starts_with('/');
  format!("file://{}{}", if absolute { "" } else { "/" }, encoded)
}

/// Compute diagnostics for a document.
///
/// * `source` – document text
/// * `path` – file path used for syntax detection and module resolution
///   (`None` for untitled documents)
pub fn compute(source: &str, path: Option<&std::path::Path>) -> Vec<Value> {
  compute_with_syntax(source, path, None)
}

/// Compute diagnostics with an explicit syntax `hint` for documents whose
/// path cannot determine the syntax (untitled editors): the editor's
/// languageId decides instead of defaulting to braces.
pub fn compute_with_syntax(
  source: &str,
  path: Option<&std::path::Path>,
  hint: Option<Syntax>,
) -> Vec<Value> {
  let syntax = path
    .filter(|p| p.extension().is_some())
    .map(vass_parser::detect_syntax)
    .or(hint)
    .unwrap_or(Syntax::Braces);

  let sheet = match vass_parser::parse(source, syntax) {
    Ok(s) => s,
    Err(e) => return vec![error_to_lsp(&e, SEVERITY_ERROR)],
  };

  let options = vass_evaluator::EvalOptions {
    entry: path.map(|p| p.to_path_buf()),
    load_paths: Vec::new(),
    loader: None,
  };
  match vass_evaluator::evaluate_with(&sheet, options) {
    Ok(_css) => Vec::new(),
    Err(e) => vec![error_to_lsp(&e, SEVERITY_ERROR)],
  }
}

fn error_to_lsp(e: &vass_core::error::Error, severity: u32) -> Value {
  let range = e
    .span
    .as_ref()
    .map(span_to_range)
    .unwrap_or(serde_json::json!({
        "start": { "line": 0, "character": 0 },
        "end": { "line": 0, "character": 1 },
    }));
  // Environmental problems (config/IO) are warnings, not language errors.
  let severity = if matches!(
    e.kind,
    vass_core::error::ErrorKind::Config | vass_core::error::ErrorKind::Io
  ) {
    SEVERITY_WARNING
  } else {
    severity
  };
  // Same stable code the terminal renderer prints (`error[E0001]`) so the
  // editor and the CLI report one identical identifier.
  let mut message = clean_message(&e.message);
  if let Some(help) = e.help() {
    message.push_str(&format!("\n\nhelp: {help}"));
  }
  for note in &e.notes {
    message.push_str(&format!("\n\nnote: {}", clean_message(&note.message)));
  }

  // relatedInformation: notes attributed to another file become clickable
  // locations (e.g. the nearby `_base.scss` an E0008 hints at).
  let related: Vec<Value> = e
    .notes
    .iter()
    .filter_map(|n| {
      let file = n.file.as_ref()?;
      let range = n
        .span
        .as_ref()
        .map(span_to_range)
        .unwrap_or(serde_json::json!({
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 1 },
        }));
      Some(serde_json::json!({
          "location": { "uri": path_to_uri(file), "range": range },
          "message": n.message,
      }))
    })
    .collect();

  // Quick fix data: the codeAction handler turns this into a WorkspaceEdit
  let fix = e.fix().map(|f| match &f.kind {
    FixKind::Replace(text) => serde_json::json!({
        "title": f.title,
        "replace": text,
    }),
    FixKind::Insert { line, column, text } => serde_json::json!({
        "title": f.title,
        "insert": {
            "line": line.saturating_sub(1),
            "character": column.saturating_sub(1),
            "text": text,
        },
    }),
  });

  let mut diag = serde_json::json!({
      "range": range,
      "severity": severity,
      "source": "vass",
      "message": message,
      "code": e.code_or_default(),
  });
  if !related.is_empty() {
    diag["relatedInformation"] = Value::Array(related);
  }
  if let Some(fix) = fix {
    diag["data"] = serde_json::json!({ "fix": fix });
  }
  diag
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn valid_document_has_no_diagnostics() {
    let d = compute(".a { color: red; }", None);
    assert!(d.is_empty(), "{d:?}");
  }

  #[test]
  fn undefined_variable_is_reported_with_position() {
    let src = ".a {\n  color: $nope;\n}\n";
    let path = std::path::Path::new("/tmp/test.vcss");
    let d = compute(src, Some(path));
    assert_eq!(d.len(), 1);
    assert_eq!(d[0]["severity"], SEVERITY_ERROR);
    assert_eq!(d[0]["source"], "vass");
    let msg = d[0]["message"].as_str().unwrap().to_lowercase();
    assert!(msg.contains("nope") || msg.contains("variable"), "{msg}");
    // line 2 (0-based 1)
    assert_eq!(
      d[0]["range"]["start"]["line"].as_u64().unwrap(),
      1,
      "range {}",
      d[0]["range"]
    );
  }

  #[test]
  fn parse_error_reported() {
    let src = ".a { color: }";
    let d = compute(src, Some(std::path::Path::new("/tmp/x.vcss")));
    assert!(!d.is_empty());
  }

  #[test]
  fn diagnostic_carries_stable_code_and_help() {
    // Trigger an undefined-variable error and check the shared model:
    // E-code identifier + optional help text appended to the message.
    let src = ".a {\n  color: $nope;\n}\n";
    let d = compute(src, Some(std::path::Path::new("/tmp/t.vcss")));
    assert_eq!(d.len(), 1);
    let code = d[0]["code"].as_str().unwrap();
    assert!(code.starts_with('E') && code.len() == 5, "code {code}");
    assert_eq!(d[0]["source"], "vass");
  }

  #[test]
  fn indented_syntax_supported() {
    let src = ".a\n  color: red\n";
    let d = compute(src, Some(std::path::Path::new("/tmp/x.vass")));
    assert!(d.is_empty(), "{d:?}");
  }

  #[test]
  fn namespace_error_carries_help_and_insert_fix() {
    // `base.$v` with no `@use` anywhere: E0011 + add-@use quick fix
    let src = ".a { x: base.$v; }\n";
    let d = compute(src, Some(std::path::Path::new("/tmp/proj/main.vcss")));
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0]["code"], "E0011");
    let msg = d[0]["message"].as_str().unwrap();
    assert!(msg.contains("help: this file loads no modules"), "{msg}");
    let fix = &d[0]["data"]["fix"];
    assert_eq!(fix["title"], "Add @use 'base';");
    assert_eq!(fix["insert"]["text"], "@use 'base';\n");
    assert_eq!(fix["insert"]["line"], 0, "0-based insert position");
  }

  #[test]
  fn module_not_found_carries_related_information() {
    // A same-named module one level up produces an E0008 whose note
    // points at the nearby file (LSP relatedInformation).
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("base.vcss"), "$v: 1;\n").unwrap();
    let src_dir = dir.path().join("src");
    std::fs::create_dir(&src_dir).unwrap();
    let entry = src_dir.join("main.vcss");
    std::fs::write(&entry, "@use 'base';\n.a { x: base.$v; }\n").unwrap();
    let d = compute("@use 'base';\n.a { x: base.$v; }\n", Some(&entry));
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0]["code"], "E0008");
    let msg = d[0]["message"].as_str().unwrap();
    assert!(msg.contains("found `"), "{msg}");
    let related = d[0]["relatedInformation"].as_array().expect("related");
    assert_eq!(related.len(), 1);
    assert!(
      related[0]["location"]["uri"]
        .as_str()
        .unwrap()
        .ends_with("base.vcss")
    );
  }

  #[test]
  fn path_to_uri_handles_windows_and_spaces() {
    assert_eq!(
      path_to_uri("D:\\proj\\my dir\\a.vcss"),
      "file:///D:/proj/my%20dir/a.vcss"
    );
    assert_eq!(path_to_uri("/tmp/a.vcss"), "file:///tmp/a.vcss");
  }
}
