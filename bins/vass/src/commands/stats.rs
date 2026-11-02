//! `vass stats` — compile a file and print statistics about the result.
//!
//! Reports output size, rule/at-rule/comment/declaration counts and wall
//! time. Useful for quick performance and output-shape inspection.

use std::path::PathBuf;
use std::time::Instant;

use clap::Args as ClapArgs;
use vass_core::ast::css::{CssChild, CssNode};

/// Print compile statistics for a file
#[derive(ClapArgs, Debug)]
pub struct Args {
  /// Input VASS file
  #[arg(value_name = "INPUT")]
  pub input: String,
}

/// Aggregate counters from a compiled stylesheet
#[derive(Debug, Default)]
struct Stats {
  rules: usize,
  at_rules: usize,
  declarations: usize,
  comments: usize,
  raw: usize,
  selectors: usize,
}

impl Stats {
  fn visit_nodes(&mut self, nodes: &[CssNode]) {
    for node in nodes {
      match node {
        CssNode::Rule {
          selector, children, ..
        } => {
          self.rules += 1;
          self.selectors += selector.split(',').count();
          self.visit_children(children);
        }
        CssNode::AtRule {
          children, nodes, ..
        } => {
          self.at_rules += 1;
          self.visit_children(children);
          self.visit_nodes(nodes);
        }
        CssNode::Comment(..) => self.comments += 1,
        CssNode::Raw(..) => self.raw += 1,
      }
    }
  }

  fn visit_children(&mut self, children: &[CssChild]) {
    for child in children {
      match child {
        CssChild::Decl(..) => self.declarations += 1,
        CssChild::Comment(..) => self.comments += 1,
      }
    }
  }
}

/// Run the stats command
pub async fn run(args: Args) -> anyhow::Result<()> {
  let input = PathBuf::from(&args.input);

  let start = Instant::now();
  let source = vass_io::read_to_string(&input).map_err(|e| anyhow::anyhow!("{}", e))?;
  let syntax = vass_parser::detect_syntax(&input);
  let sheet = vass_parser::parse(&source, syntax).map_err(|e| anyhow::anyhow!("{e}"))?;
  let options = vass_evaluator::EvalOptions {
    entry: Some(input.clone()),
    load_paths: Vec::new(),
    loader: None,
  };
  let css = vass_evaluator::evaluate_with(&sheet, options).map_err(|e| anyhow::anyhow!("{e}"))?;

  let mut st = Stats::default();
  st.visit_nodes(&css.nodes);

  let expanded = vass_transpiler::render(&css, vass_transpiler::OutputStyle::Expanded);
  let compressed = vass_transpiler::render(&css, vass_transpiler::OutputStyle::Compressed);
  let elapsed = start.elapsed();

  println!("file:          {}", input.display());
  println!(
    "syntax:        {}",
    if syntax == vass_parser::Syntax::Braces {
      "braces (.vcss)"
    } else {
      "indented (.vass)"
    }
  );
  println!("source bytes:  {}", source.len());
  println!(
    "output bytes:  {} expanded / {} compressed",
    expanded.len(),
    compressed.len()
  );
  println!("rules:         {}", st.rules);
  println!("at-rules:      {}", st.at_rules);
  println!("selectors:     {}", st.selectors);
  println!("declarations:  {}", st.declarations);
  println!("comments:      {}", st.comments);
  println!("raw passthrough: {}", st.raw);
  println!("compile time:  {:.2?}", elapsed);
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_stats_new() {
    let s = Stats::default();
    assert_eq!(s.rules, 0);
    assert_eq!(s.at_rules, 0);
    assert_eq!(s.declarations, 0);
    assert_eq!(s.comments, 0);
    assert_eq!(s.raw, 0);
    assert_eq!(s.selectors, 0);
  }

  #[test]
  fn test_stats_visit_nodes_rule() {
    let mut s = Stats::default();
    let nodes = vec![CssNode::Rule {
      selector: ".a,.b".to_string(),
      children: vec![CssChild::decl("color", "red")],
      group: 0,
      open_line: 1,
      close_line: 2,
    }];
    s.visit_nodes(&nodes);
    assert_eq!(s.rules, 1);
    assert_eq!(s.selectors, 2);
    assert_eq!(s.declarations, 1);
  }

  #[test]
  fn test_stats_visit_nodes_at_rule() {
    let mut s = Stats::default();
    let nodes = vec![CssNode::AtRule {
      header: "@media".to_string(),
      children: vec![CssChild::decl("color", "red")],
      nodes: vec![],
      group: 0,
      open_line: 1,
      close_line: 2,
    }];
    s.visit_nodes(&nodes);
    assert_eq!(s.at_rules, 1);
    assert_eq!(s.declarations, 1);
  }

  #[test]
  fn test_stats_visit_nodes_comment() {
    let mut s = Stats::default();
    let nodes = vec![CssNode::Comment("test comment".to_string(), 1)];
    s.visit_nodes(&nodes);
    assert_eq!(s.comments, 1);
  }

  #[test]
  fn test_stats_visit_nodes_raw() {
    let mut s = Stats::default();
    let nodes = vec![CssNode::Raw("raw content".to_string(), 1)];
    s.visit_nodes(&nodes);
    assert_eq!(s.raw, 1);
  }

  #[test]
  fn test_stats_visit_children_decl() {
    let mut s = Stats::default();
    let children = vec![CssChild::decl("color", "red")];
    s.visit_children(&children);
    assert_eq!(s.declarations, 1);
  }

  #[test]
  fn test_stats_visit_children_comment() {
    let mut s = Stats::default();
    let children = vec![CssChild::Comment("test".to_string(), 1)];
    s.visit_children(&children);
    assert_eq!(s.comments, 1);
  }

  #[tokio::test]
  async fn test_stats_run_nonexistent_file() {
    let result = run(Args {
      input: "nonexistent_file_for_stats_test.vass".to_string(),
    })
    .await;
    assert!(result.is_err());
  }
}
