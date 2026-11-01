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
