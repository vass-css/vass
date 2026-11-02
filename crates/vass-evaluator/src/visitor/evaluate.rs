//! The main evaluator: VASS AST -> CSS AST
//!
//! Implements the full VASS language core: variables, nesting with `&`,
//! at-rule bubbling, mixins with `@content`, functions, control flow,
//! interpolation, `@extend`, placeholder selectors, custom properties,
//! nested properties, modules (`@use`/`@forward`/`@import`), and the
//! built-in function library.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use vass_core::ast::css::{CssChild, CssDecl, CssNode, CssStylesheet};
use vass_core::ast::input::expression::{BinaryOp, CallArg, Expr, Param, StrPart, UnaryOp};
use vass_core::ast::input::statement::{IfChain, Statement, Stylesheet, UseRule};
use vass_core::error::{Error, FixKind, Result};
use vass_core::value::{
  ArgListValue, Color, ColorSpace, FunctionRef, ListValue, Number, Separator, Value, VassMap,
  VassString, values_equal,
};

use crate::builtins;
use crate::environment::{Environment, Module, UserCallable};

/// Evaluated call arguments: (positional, named, rest)
type EvaluatedArgs = (Vec<Value>, Vec<(String, Value)>, Option<Value>);

/// Maximum recursion depth for function/mixin calls
const MAX_CALL_DEPTH: usize = 300;

/// Options for evaluation: how to resolve `@use`/`@import` targets
#[derive(Default)]
pub struct EvalOptions {
  /// The entry file (enables relative module resolution)
  pub entry: Option<PathBuf>,
  /// Extra directories searched for imports/uses
  pub load_paths: Vec<PathBuf>,
  /// A custom file reader (defaults to `std::fs::read_to_string`)
  pub loader: Option<LoaderFn>,
}

/// A custom file reader function
pub type LoaderFn = Box<dyn Fn(&Path) -> std::io::Result<String>>;

impl std::fmt::Debug for EvalOptions {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("EvalOptions")
      .field("entry", &self.entry)
      .field("load_paths", &self.load_paths)
      .finish()
  }
}

/// A content block captured for `@content` (closes over the include site's
/// variable scopes)
struct ContentBlock {
  body: Vec<Statement>,
  params: Vec<Param>,
  scopes: Vec<HashMap<String, Value>>,
}

/// One pending `@extend` record
#[derive(Debug, Clone)]
struct ExtendRecord {
  /// The simple selector being extended (`.error`, `%ph`, `a`)
  target: String,
  /// The extending selector (may be a comma list)
  extender: String,
  /// `@extend target !optional` — suppresses the not-found error
  optional: bool,
}

/// Compile context threaded through statement compilation
#[derive(Clone, Default)]
struct Ctx {
  /// Current selector list (empty at top level)
  parents: Vec<String>,
  /// Inside `@keyframes` (selectors are not prefixed)
  in_keyframes: bool,
  /// Enclosing `@media` condition texts (for merging on nesting)
  media_stack: Vec<String>,
  /// Whether the current context ignores parents (font-face etc.)
  no_parents: bool,
}

/// Evaluate a parsed stylesheet into a CSS stylesheet (no file loading)
pub fn evaluate(sheet: &Stylesheet) -> Result<CssStylesheet> {
  evaluate_with(sheet, EvalOptions::default())
}

/// Evaluate a parsed stylesheet with module-loading options
pub fn evaluate_with(sheet: &Stylesheet, options: EvalOptions) -> Result<CssStylesheet> {
  let mut ev = Evaluator::new(options);
  if let Some(entry) = ev.options.entry.clone() {
    ev.current_file.push(entry);
  }
  let mut out = ev.load_use_modules(sheet)?;
  let mut nodes = Vec::new();
  // Top-level @use/@forward were already processed above; the compile loop
  // must not process them a second time (a configured re-use would
  // falsely trip the already-loaded check)
  ev.root_uses_done = true;
  ev.compile_stmts_root(&sheet.body, &Ctx::default(), &mut nodes)?;
  apply_extends(&mut nodes, &ev.extends, &ev.seen_selectors)?;
  out.append(&mut nodes);
  Ok(CssStylesheet { nodes: out })
}

/// The evaluator: carries scopes, modules, extends and content blocks
struct Evaluator {
  env: Environment,
  extends: Vec<ExtendRecord>,
  content: Vec<Option<ContentBlock>>,
  options: EvalOptions,
  /// Stack of files currently being compiled (for relative resolution)
  current_file: Vec<PathBuf>,
  /// Module cache by canonical path
  module_cache: HashMap<PathBuf, Rc<Module>>,
  /// Whether top-level @use/@forward rules were pre-processed (the root
  /// compile loop then skips them)
  root_uses_done: bool,
  /// >0 while a module body is being compiled (root sheet: 0)
  module_nesting: usize,
  /// >0 while compiling a legacy-imported stylesheet
  import_nesting: usize,
  /// Whether the most recent load_module call performed a fresh load (vs a
  /// cache hit) — the reference emits a module's CSS only on first load
  just_loaded: bool,
  /// Module load stack (cycle detection)
  loading: Vec<PathBuf>,
  /// CSS nodes from used modules, in use order
  module_css: Vec<CssNode>,
  /// Current call depth
  depth: usize,
  /// Parameter defaults deferred to callee scope
  pending_defaults: Vec<Param>,
  /// Whether variable assignments write to the innermost scope only
  /// (inside style-rule bodies, per reference shadowing semantics)
  local_assign: bool,
  /// Namespace alias -> real built-in module name (`m` -> `math`)
  builtin_ns: HashMap<String, String>,
  /// Monotonic group id stamped on root-level nodes: nodes from the same
  /// source statement share a group; the writer blank-lines only between
  /// different groups
  group: u32,
  /// Non-null while a function body is executing (captures `@return`)
  return_value: Option<Value>,
  /// Every selector text seen during compilation (including rules later
  /// suppressed as empty) — used to validate `@extend` targets
  seen_selectors: Vec<String>,
}

impl Evaluator {
  fn new(options: EvalOptions) -> Self {
    Evaluator {
      env: Environment::new(),
      extends: Vec::new(),
      content: Vec::new(),
      options,
      current_file: Vec::new(),
      module_cache: HashMap::new(),
      just_loaded: false,
      root_uses_done: false,
      module_nesting: 0,
      import_nesting: 0,
      loading: Vec::new(),
      module_css: Vec::new(),
      depth: 0,
      pending_defaults: Vec::new(),
      local_assign: false,
      builtin_ns: HashMap::new(),
      group: 0,
      return_value: None,
      seen_selectors: Vec::new(),
    }
  }

  /// Pre-load `@use` modules so their CSS lands before this sheet's output
  fn load_use_modules(&mut self, sheet: &Stylesheet) -> Result<Vec<CssNode>> {
    for stmt in &sheet.body {
      if let Statement::Use(rule) = stmt {
        self.do_use(rule)?;
      }
    }
    Ok(std::mem::take(&mut self.module_css))
  }

  // ---- statement compilation ------------------------------------------

  fn compile_stmts(
    &mut self,
    stmts: &[Statement],
    ctx: &Ctx,
    out: &mut Vec<CssNode>,
  ) -> Result<Vec<CssChild>> {
    self.env.push_scope();
    let mut children: Vec<CssChild> = Vec::new();
    let result = self.compile_stmts_inner(stmts, ctx, out, &mut children);
    self.env.pop_scope();
    result?;
    Ok(children)
  }

  /// Compile ROOT-level statements directly in the global scope (no new
  /// scope): top-level variables must land in the global scope
  fn compile_stmts_root(
    &mut self,
    stmts: &[Statement],
    ctx: &Ctx,
    out: &mut Vec<CssNode>,
  ) -> Result<Vec<CssChild>> {
    let mut children: Vec<CssChild> = Vec::new();
    self.compile_stmts_inner(stmts, ctx, out, &mut children)?;
    Ok(children)
  }

  /// Compile statements WITHOUT pushing a scope (control flow shares the
  /// enclosing scope, per VASS semantics)
  fn compile_stmts_same_scope(
    &mut self,
    stmts: &[Statement],
    ctx: &Ctx,
    out: &mut Vec<CssNode>,
  ) -> Result<Vec<CssChild>> {
    let mut children: Vec<CssChild> = Vec::new();
    self.compile_stmts_inner(stmts, ctx, out, &mut children)?;
    Ok(children)
  }

  fn compile_stmts_inner(
    &mut self,
    stmts: &[Statement],
    ctx: &Ctx,
    out: &mut Vec<CssNode>,
    children: &mut Vec<CssChild>,
  ) -> Result<()> {
    for stmt in stmts {
      // Each root-level source statement starts a new output group:
      // the writer blank-lines between root nodes of different groups
      if ctx.parents.is_empty() && !ctx.in_keyframes && ctx.media_stack.is_empty() {
        self.group += 1;
      }
      match stmt {
        Statement::VariableDecl {
          name,
          value,
          guarded,
          global,
        } => {
          if *guarded && self.env.is_defined(name) {
            continue;
          }
          let v = self.eval_expr(value)?;
          if *global {
            self.env.set(name, v, true);
          } else if self.local_assign {
            if self.env.is_defined_in_local_scopes(name) {
              // The reference UPDATES a variable that already
              // exists in an enclosing local scope (mixin ->
              // rule/@each/@for/@if), while global shadowing
              // stays local (probe-verified)
              self.env.set(name, v, false);
            } else {
              // Style-rule bodies shadow globals (reference
              // semantics: assignments stay local)
              self.env.define_local(name, v);
            }
          } else {
            self.env.set(name, v, false);
          }
        }
        Statement::Declaration {
          prop,
          value,
          important,
          children: prop_children,
          custom,
          custom_raw,
          end_line,
        } => {
          if ctx.parents.is_empty() && !ctx.no_parents && !ctx.in_keyframes {
            return Err(Error::syntax(format!(
              "Declaration `{}` may only be used within a style rule.",
              prop
            )));
          }
          self.emit_declaration(
            prop,
            value,
            *important,
            prop_children,
            *custom,
            custom_raw.as_deref(),
            *end_line,
            ctx,
            children,
          )?;
        }
        Statement::Comment { text, line } => {
          if ctx.parents.is_empty() {
            out.push(CssNode::Comment(text.clone(), *line));
          } else {
            children.push(CssChild::Comment(text.clone(), *line));
          }
        }
        Statement::AtRuleStmt { raw, line } => {
          let raw = self.interpolate_string(raw)?;
          // `@charset` is consumed by the reference: output carries
          // one only when the CSS is non-ASCII (added by the
          // serializer), so a source @charset never passes through
          if raw.trim_start().starts_with("@charset") {
            continue;
          }
          out.push(CssNode::Raw(raw, *line));
        }
        Statement::StyleRule {
          selector,
          body,
          open_line,
          close_line,
        } => {
          self.compile_style_rule(selector, body, *open_line, *close_line, ctx, out)?;
        }
        Statement::AtRule {
          name,
          params,
          body,
          open_line,
          close_line,
        } => {
          self.compile_at_rule(name, params, body, *open_line, *close_line, ctx, out)?;
        }
        Statement::MixinDecl(decl) => {
          self.env.define_callable(decl, true);
        }
        Statement::FunctionDecl(decl) => {
          self.env.define_callable(decl, false);
        }
        Statement::Include {
          name,
          args,
          content,
          using,
        } => {
          let block = content.as_ref().map(|body| ContentBlock {
            body: body.clone(),
            params: using.clone(),
            scopes: self.env.snapshot(),
          });
          self.compile_include(name, args, block, ctx, out, children)?;
        }
        Statement::Content { args } => {
          self.compile_content(args, ctx, out, children)?;
        }
        Statement::If(chain) => {
          self.compile_if(chain, ctx, out, children)?;
        }
        Statement::Each { vars, list, body } => {
          self.compile_each(vars, list, body, ctx, out, children)?;
        }
        Statement::For {
          var,
          from,
          to,
          inclusive,
          body,
        } => {
          self.compile_for(var, from, to, *inclusive, body, ctx, out, children)?;
        }
        Statement::While { cond, body } => {
          self.compile_while(cond, body, ctx, out, children)?;
        }
        Statement::Return { value } => {
          if self.return_value.is_none() {
            let v = self.eval_expr(value)?;
            self.return_value = Some(v);
          }
        }
        Statement::Extend {
          target, optional, ..
        } => {
          if ctx.parents.is_empty() && !ctx.no_parents {
            return Err(Error::syntax(
              "@extend may only be used within a style rule.",
            ));
          }
          let resolved = self.interpolate_string(target)?;
          for parent in &ctx.parents {
            self.extends.push(ExtendRecord {
              target: resolved.clone(),
              extender: parent.clone(),
              optional: *optional,
            });
          }
        }
        Statement::AtRoot { query, body } => {
          self.compile_at_root(query.as_deref(), body, ctx, out)?;
        }
        Statement::Debug { value } => {
          let v = self.eval_expr(value)?;
          let file = self
            .current_file
            .last()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<stdin>".to_string());
          eprintln!("{} DEBUG: {}", file, v.to_debug_string());
        }
        Statement::Warn { value } => {
          let v = self.eval_expr(value)?;
          eprintln!("WARNING: {}", v.to_debug_string());
        }
        Statement::Error { value } => {
          let v = self.eval_expr(value)?;
          return Err(Error::runtime(v.inspect()));
        }
        Statement::Use(rule) => {
          // dart-sass: `@use` is only legal at a stylesheet's top
          // level (the .vcss parser rejects it earlier; this guard
          // covers indented-syntax and module-body paths)
          if !ctx.parents.is_empty() || !ctx.media_stack.is_empty() {
            return Err(Error::syntax("This at-rule is not allowed here."));
          }
          // pre-processed for the root sheet before this loop
          if self.root_uses_done && self.module_nesting == 0 && self.import_nesting == 0 {
            continue;
          }
          self.do_use(rule)?;
        }
        Statement::Forward(rule) => {
          if !ctx.parents.is_empty() || !ctx.media_stack.is_empty() {
            return Err(Error::syntax("This at-rule is not allowed here."));
          }
          if self.root_uses_done && self.module_nesting == 0 && self.import_nesting == 0 {
            continue;
          }
          self.do_forward(rule)?;
        }
        Statement::Import(targets) => {
          self.compile_import(targets, ctx, out, children)?;
        }
      }
    }
    Ok(())
  }

  /// Emit one declaration (with nested-property children) into `children`
  #[allow(clippy::too_many_arguments)]
  fn emit_declaration(
    &mut self,
    prop: &str,
    value: &Option<Expr>,
    important: bool,
    prop_children: &[Statement],
    custom: bool,
    custom_raw: Option<&str>,
    end_line: u32,
    _ctx: &Ctx,
    children: &mut Vec<CssChild>,
  ) -> Result<()> {
    let prop = self.interpolate_string(prop)?;
    // Custom properties: raw text with interpolation only, never omitted
    if custom {
      let raw = custom_raw.unwrap_or("");
      let text = self.interpolate_string(raw)?;
      children.push(CssChild::Decl(CssDecl { prop, value: text }, end_line));
      return Ok(());
    }
    if let Some(v) = value {
      let val = self.eval_expr(v)?;
      // An empty unbracketed list has no CSS representation
      // (reference: "() isn't a valid CSS value.")
      if let Value::List(l) = &val
        && l.items.is_empty()
        && !l.bracketed
      {
        return Err(Error::type_error("() isn't a valid CSS value."));
      }
      if !matches!(val, Value::Null) {
        let mut text = val.to_css();
        if important {
          text.push_str(" !important");
        }
        children.push(CssChild::Decl(
          CssDecl {
            prop: prop.clone(),
            value: text,
          },
          end_line,
        ));
      }
    }
    // Nested properties: `font: 12px { family: serif }`
    for child in prop_children {
      if let Statement::Declaration {
        prop: cprop,
        value: cvalue,
        important: cimportant,
        children: cchildren,
        custom: ccustom,
        custom_raw: ccustom_raw,
        end_line: cend_line,
      } = child
      {
        let full_prop = format!("{}-{}", prop, cprop);
        if *ccustom {
          let text = self.interpolate_string(ccustom_raw.as_deref().unwrap_or(""))?;
          children.push(CssChild::Decl(
            CssDecl {
              prop: full_prop,
              value: text,
            },
            *cend_line,
          ));
          continue;
        }
        if let Some(cv) = cvalue {
          let val = self.eval_expr(cv)?;
          if !matches!(val, Value::Null) {
            let mut text = val.to_css();
            if *cimportant {
              text.push_str(" !important");
            }
            children.push(CssChild::Decl(
              CssDecl {
                prop: full_prop.clone(),
                value: text,
              },
              *cend_line,
            ));
          }
        }
        if !cchildren.is_empty() {
          // deeper nesting flattens recursively
          let nested = Statement::Declaration {
            prop: full_prop.clone(),
            value: None,
            important: false,
            children: cchildren.clone(),
            custom: false,
            custom_raw: None,
            end_line: *cend_line,
          };
          let mut sub_children = Vec::new();
          self.emit_nested(&nested, &mut sub_children)?;
          children.append(&mut sub_children);
        }
      }
    }
    Ok(())
  }

  /// Emit a nested (compound) property declaration
  fn emit_nested(&mut self, stmt: &Statement, children: &mut Vec<CssChild>) -> Result<()> {
    if let Statement::Declaration {
      prop,
      value,
      important,
      children: prop_children,
      custom,
      custom_raw,
      end_line,
    } = stmt
    {
      self.emit_declaration(
        prop,
        value,
        *important,
        prop_children,
        *custom,
        custom_raw.as_deref(),
        *end_line,
        &Ctx::default(),
        children,
      )
    } else {
      Ok(())
    }
  }

  /// A style rule: resolve selectors, recurse
  #[allow(clippy::too_many_arguments)]
  fn compile_style_rule(
    &mut self,
    selector: &str,
    body: &[Statement],
    open_line: u32,
    close_line: u32,
    ctx: &Ctx,
    out: &mut Vec<CssNode>,
  ) -> Result<()> {
    let raw = self.interpolate_string(selector)?;
    let new_parents = if ctx.in_keyframes || ctx.no_parents {
      validate_parent_positions(&raw)?;
      raw
        .split(',')
        .map(|s| s.trim().to_string())
        .collect::<Vec<_>>()
    } else {
      resolve_child_selectors(&ctx.parents, &raw)?
    };
    if new_parents.iter().all(|s| s.is_empty()) {
      return Ok(());
    }
    let child_ctx = Ctx {
      parents: new_parents,
      in_keyframes: ctx.in_keyframes,
      media_stack: ctx.media_stack.clone(),
      no_parents: false,
    };
    // Assignments inside a style rule always write to the rule's scope
    // (shadowing), matching reference semantics
    let prev_local = self.local_assign;
    self.local_assign = true;
    // Compile statement-by-statement so hoisted nested rules keep their
    // SOURCE position relative to the parent's own children: a nested
    // rule appearing before the first child is emitted before the
    // parent's block (`y { z { a: b } /* c */ }` -> `y z {...}` first,
    // then `y { /* c */ }`), matching the reference serializer. The
    // scope is pushed ONCE for the whole body (variables declared in
    // one statement stay visible in later ones).
    //
    // Declarations that FOLLOW a nested rule start a NEW rule block with
    // the same selector (reference behaviour:
    // `.a { c: red; .b { x } m: 0 }` -> `.a { c: red } .a .b { x }
    // .a { m: 0 }`), so declaration groups are flushed at every
    // nested-node boundary and everything stays in source order.
    self.env.push_scope();
    let mut pending_children: Vec<CssChild> = Vec::new();
    let mut segments: Vec<CssNode> = Vec::new();
    let pre_nodes: Vec<CssNode> = Vec::new();
    let mut compile_result: Result<()> = Ok(());
    for stmt in body {
      let mut stmt_nodes = Vec::new();
      let mut stmt_children: Vec<CssChild> = Vec::new();
      if let Err(e) = self.compile_stmts_inner(
        std::slice::from_ref(stmt),
        &child_ctx,
        &mut stmt_nodes,
        &mut stmt_children,
      ) {
        compile_result = Err(e);
        break;
      }
      if !stmt_children.is_empty() {
        pending_children.extend(stmt_children);
      }
      if !stmt_nodes.is_empty() {
        if !pending_children.is_empty() {
          let taken = std::mem::take(&mut pending_children);
          segments.push(CssNode::Rule {
            selector: child_ctx.parents.join(", "),
            children: taken,
            group: self.group,
            open_line,
            close_line,
          });
        }
        segments.extend(stmt_nodes);
      }
    }
    self.env.pop_scope();
    self.local_assign = prev_local;
    compile_result?;
    // The trailing declaration group is the parent's main block
    let sub_children = pending_children;
    let sub_nodes = segments;
    // Emit the rule if it has content; placeholder-only rules are kept
    // for now and filtered after @extend resolution
    // Record every selector for `@extend` target validation, including
    // rules that may be suppressed as empty below
    let seen_sel = child_ctx.parents.join(", ");
    self.seen_selectors.push(seen_sel);
    let is_placeholder = child_ctx.parents.iter().all(|p| {
      p.split_whitespace()
        .next()
        .is_some_and(|c| c.starts_with('%'))
    });
    let _ = is_placeholder;
    // Source order: hoisted pre-nodes, then the interleaved segments,
    // then the trailing declaration group as the parent's main block
    out.extend(pre_nodes);
    out.extend(sub_nodes);
    if !sub_children.is_empty() {
      out.push(CssNode::Rule {
        selector: child_ctx.parents.join(", "),
        children: sub_children,
        group: self.group,
        open_line,
        close_line,
      });
    }
    Ok(())
  }

  /// A block at-rule: `@media` bubbling + merging, keyframes, containers
  #[allow(clippy::too_many_arguments)]
  fn compile_at_rule(
    &mut self,
    name: &str,
    params: &str,
    body: &[Statement],
    open_line: u32,
    close_line: u32,
    ctx: &Ctx,
    out: &mut Vec<CssNode>,
  ) -> Result<()> {
    // `@media` preludes are evaluated structurally: the media type passes
    // through, feature values evaluate (variables, interpolation,
    // arithmetic) and the query list is re-serialized in the reference
    // compiler's normalized form
    let params = if name == "media" {
      self.eval_media_prelude(params)?
    } else {
      self.interpolate_string(params)?
    };
    let header = if params.is_empty() {
      format!("@{}", name)
    } else {
      format!("@{} {}", name, params)
    };
    if name.ends_with("keyframes") {
      let child_ctx = Ctx {
        parents: vec![],
        in_keyframes: true,
        media_stack: ctx.media_stack.clone(),
        no_parents: false,
      };
      let mut sub_nodes = Vec::new();
      let sub = self.compile_stmts(body, &child_ctx, &mut sub_nodes)?;
      if !sub.is_empty() || !sub_nodes.is_empty() {
        out.push(CssNode::AtRule {
          header,
          children: sub,
          nodes: sub_nodes,
          group: self.group,
          open_line,
          close_line,
        });
      }
      return Ok(());
    }

    // Contained at-rules: no parent selectors inside
    if matches!(
      name,
      "font-face"
        | "page"
        | "viewport"
        | "-ms-viewport"
        | "counter-style"
        | "font-feature-values"
        | "property"
        | "layer"
    ) {
      let child_ctx = Ctx {
        parents: vec![],
        in_keyframes: ctx.in_keyframes,
        media_stack: ctx.media_stack.clone(),
        no_parents: true,
      };
      let mut sub_nodes = Vec::new();
      let sub = self.compile_stmts(body, &child_ctx, &mut sub_nodes)?;
      if !sub.is_empty() || !sub_nodes.is_empty() {
        out.push(CssNode::AtRule {
          header,
          children: sub,
          nodes: sub_nodes,
          group: self.group,
          open_line,
          close_line,
        });
      }
      return Ok(());
    }

    // Bubble above the current selectors when inside a rule
    if !ctx.parents.is_empty() && !ctx.no_parents && !ctx.in_keyframes {
      return self.bubble_at_rule(
        name, &header, &params, body, open_line, close_line, ctx, out,
      );
    }

    // Regular container (top level or inside another at-rule);
    // nested `@media` merges with its parent via `and`
    if name == "media"
      && let Some(outer) = ctx.media_stack.last().cloned()
    {
      let merged = format!("{} and {}", outer, params);
      let mut merged_stack = ctx.media_stack.clone();
      merged_stack.pop();
      merged_stack.push(merged.clone());
      let child_ctx = Ctx {
        parents: ctx.parents.clone(),
        in_keyframes: ctx.in_keyframes,
        media_stack: merged_stack,
        no_parents: false,
      };
      let mut sub_nodes = Vec::new();
      let sub = self.compile_stmts(body, &child_ctx, &mut sub_nodes)?;
      if !sub.is_empty() || !sub_nodes.is_empty() {
        out.push(CssNode::AtRule {
          header: format!("@media {}", merged),
          children: sub,
          nodes: sub_nodes,
          group: self.group,
          open_line,
          close_line,
        });
      }
      return Ok(());
    }
    let media_stack = if name == "media" {
      let mut ms = ctx.media_stack.clone();
      ms.push(params);
      ms
    } else {
      ctx.media_stack.clone()
    };
    let child_ctx = Ctx {
      parents: ctx.parents.clone(),
      in_keyframes: ctx.in_keyframes,
      media_stack,
      no_parents: false,
    };
    let mut sub_nodes = Vec::new();
    let sub = self.compile_stmts(body, &child_ctx, &mut sub_nodes)?;
    if !sub.is_empty() || !sub_nodes.is_empty() {
      out.push(CssNode::AtRule {
        header,
        children: sub,
        nodes: sub_nodes,
        group: self.group,
        open_line,
        close_line,
      });
    }
    Ok(())
  }

  /// Bubble an at-rule found inside a style rule:
  /// `.a { @media x { color: red; .b { c: d } } }` becomes
  /// `@media x { .a { color: red } .a .b { c: d } }`
  #[allow(clippy::too_many_arguments)]
  fn bubble_at_rule(
    &mut self,
    name: &str,
    header: &str,
    params: &str,
    body: &[Statement],
    open_line: u32,
    close_line: u32,
    ctx: &Ctx,
    out: &mut Vec<CssNode>,
  ) -> Result<()> {
    // Merge nested `@media` queries with `and`
    if name == "media"
      && let Some(outer) = ctx.media_stack.last().cloned()
    {
      let merged = format!("{} and {}", outer, params);
      let mut merged_stack = ctx.media_stack.clone();
      merged_stack.pop();
      merged_stack.push(merged.clone());
      let child_ctx = Ctx {
        parents: ctx.parents.clone(),
        in_keyframes: ctx.in_keyframes,
        media_stack: merged_stack,
        no_parents: false,
      };
      let mut sub_nodes = Vec::new();
      let sub = self.compile_stmts(body, &child_ctx, &mut sub_nodes)?;
      let mut inner = Vec::new();
      if !sub.is_empty() {
        // Direct declarations belong to the parent selectors
        inner.push(CssNode::Rule {
          selector: ctx.parents.join(", "),
          children: sub,
          group: self.group,
          open_line,
          close_line,
        });
      }
      inner.extend(sub_nodes);
      if !inner.is_empty() {
        out.push(CssNode::AtRule {
          header: format!("@media {}", merged),
          children: Vec::new(),
          nodes: inner,
          group: self.group,
          open_line,
          close_line,
        });
      }
      return Ok(());
    }

    // Generic bubbling: wrap direct declarations in a parent rule
    let mut sub_nodes = Vec::new();
    let sub = self.compile_stmts(body, ctx, &mut sub_nodes)?;
    let mut inner = Vec::new();
    if !sub.is_empty() {
      inner.push(CssNode::Rule {
        selector: ctx.parents.join(", "),
        children: sub,
        group: self.group,
        open_line,
        close_line,
      });
    }
    inner.extend(sub_nodes);
    if !inner.is_empty() {
      out.push(CssNode::AtRule {
        header: header.to_string(),
        children: Vec::new(),
        nodes: inner,
        group: self.group,
        open_line,
        close_line,
      });
    }
    Ok(())
  }

  /// `@at-root`: compile the body without the enclosing contexts
  fn compile_at_root(
    &mut self,
    query: Option<&str>,
    body: &[Statement],
    ctx: &Ctx,
    out: &mut Vec<CssNode>,
  ) -> Result<()> {
    let q = match query {
      Some(q) => self.interpolate_string(q)?,
      None => String::new(),
    };
    let without = q.contains("without");
    let with = q.contains("with:") && !without;
    // Default: (without: rule) — hoist out of style rules, keep media
    let mut drop_rule = q.is_empty() || (without && !q.contains("media") && !q.contains("all"));
    let mut drop_media = without && (q.contains("media") || q.contains("all"));
    if with {
      // (with: x) keeps only listed contexts
      drop_rule = !q.contains("rule");
      drop_media = !q.contains("media");
    }
    let child_ctx = Ctx {
      parents: if drop_rule {
        vec![]
      } else {
        ctx.parents.clone()
      },
      in_keyframes: false,
      media_stack: if drop_media {
        vec![]
      } else {
        ctx.media_stack.clone()
      },
      no_parents: false,
    };
    let mut sub_nodes = Vec::new();
    self.compile_stmts(body, &child_ctx, &mut sub_nodes)?;
    out.append(&mut sub_nodes);
    Ok(())
  }

  /// `@include`
  fn compile_include(
    &mut self,
    name: &str,
    args: &[CallArg],
    content: Option<ContentBlock>,
    ctx: &Ctx,
    out: &mut Vec<CssNode>,
    children: &mut Vec<CssChild>,
  ) -> Result<()> {
    // dart-sass 1.79+: `@include meta.apply($mixin, $args...)` includes a
    // mixin VALUE (as returned by meta.get-mixin / module-lookup helpers).
    if let Some((ns, member)) = name.split_once('.') {
      let real = self
        .builtin_ns
        .get(ns)
        .cloned()
        .unwrap_or_else(|| ns.to_string());
      if real == "meta" && member == "apply" {
        let (evaluated, _, _) = self.eval_args(args)?;
        let target = match evaluated.first() {
          Some(Value::Mixin(m)) => m.clone(),
          Some(other) => {
            return Err(Error::type_error(format!(
              "meta.apply: {} is not a mixin.",
              other.inspect()
            )));
          }
          None => return Err(Error::type_error("meta.apply: expected a mixin.")),
        };
        let rest: Vec<CallArg> = args.iter().skip(1).cloned().collect();
        let callable = self.resolve_callable(&target, true, "meta.apply")?;
        let bound = self.bind_call_args(&callable, &rest, &target)?;
        return self.invoke_mixin(&callable, bound, content, ctx, out, children);
      }
    }
    let callable = self.resolve_callable(name, true, "@include")?;
    let bound = self.bind_call_args(&callable, args, name)?;
    self.invoke_mixin(&callable, bound, content, ctx, out, children)
  }

  /// `@content`
  fn compile_content(
    &mut self,
    args: &[CallArg],
    ctx: &Ctx,
    out: &mut Vec<CssNode>,
    children: &mut Vec<CssChild>,
  ) -> Result<()> {
    let block = match self.content.last() {
      Some(Some(b)) => ContentBlock {
        body: b.body.clone(),
        params: b.params.clone(),
        scopes: b.scopes.clone(),
      },
      _ => return Ok(()), // @content with no block is a no-op
    };
    // Evaluate @content args in the mixin's current scope
    let mut vals = Vec::new();
    for arg in args {
      vals.push(self.eval_expr(&arg.expr)?);
    }
    // Restore the include site's scopes, then bind content params
    self.env.push_scopes(&block.scopes);
    for (i, param) in block.params.iter().enumerate() {
      let v = vals.get(i).cloned().unwrap_or(Value::Null);
      self.env.define_local(&param.name, v);
    }
    let body = block.body;
    let result = self.compile_stmts_same_scope(&body, ctx, out);
    self.env.pop_scopes(block.scopes.len());
    let sub = result?;
    children.extend(sub);
    Ok(())
  }

  /// `@if` / `@else if` / `@else`
  fn compile_if(
    &mut self,
    chain: &IfChain,
    ctx: &Ctx,
    out: &mut Vec<CssNode>,
    children: &mut Vec<CssChild>,
  ) -> Result<()> {
    for (cond, body) in &chain.clauses {
      let c = self.eval_expr(cond)?;
      if c.is_truthy() {
        let sub = self.compile_stmts_same_scope(body, ctx, out)?;
        children.extend(sub);
        return Ok(());
      }
    }
    if let Some(else_body) = &chain.else_body {
      let sub = self.compile_stmts_same_scope(else_body, ctx, out)?;
      children.extend(sub);
    }
    Ok(())
  }

  /// `@each $a, $b in $list`
  fn compile_each(
    &mut self,
    vars: &[String],
    list: &Expr,
    body: &[Statement],
    ctx: &Ctx,
    out: &mut Vec<CssNode>,
    children: &mut Vec<CssChild>,
  ) -> Result<()> {
    let lv = self.eval_expr(list)?;
    let (items, _, _) = builtins::to_list(&lv);
    for item in items {
      let tuple = builtins::to_list(&item).0;
      self.env.push_scope();
      for (i, var) in vars.iter().enumerate() {
        let v = if vars.len() == 1 {
          item.clone()
        } else {
          tuple.get(i).cloned().unwrap_or(Value::Null)
        };
        self.env.define_local(var, v);
      }
      let result = self.compile_stmts_same_scope(body, ctx, out);
      self.env.pop_scope();
      let sub = result?;
      children.extend(sub);
    }
    Ok(())
  }

  /// `@for $i from a through b`
  #[allow(clippy::too_many_arguments)]
  fn compile_for(
    &mut self,
    var: &str,
    from: &Expr,
    to: &Expr,
    inclusive: bool,
    body: &[Statement],
    ctx: &Ctx,
    out: &mut Vec<CssNode>,
    children: &mut Vec<CssChild>,
  ) -> Result<()> {
    const F: &str = "@for";
    let f = self.eval_expr(from)?;
    let t = self.eval_expr(to)?;
    let fv = match &f {
      Value::Number(n) => n.value.round() as i64,
      _ => return Err(Error::type_error(format!("{}: $from must be a number.", F))),
    };
    let tv = match &t {
      Value::Number(n) => n.value.round() as i64,
      _ => return Err(Error::type_error(format!("{}: $to must be a number.", F))),
    };
    let step = if fv <= tv { 1 } else { -1 };
    let end = if inclusive { tv } else { tv - step };
    let mut i = fv;
    while (step > 0 && i <= end) || (step < 0 && i >= end) {
      self.env.push_scope();
      self
        .env
        .define_local(var, Value::Number(Number::unitless(i as f64)));
      let result = self.compile_stmts_same_scope(body, ctx, out);
      self.env.pop_scope();
      let sub = result?;
      children.extend(sub);
      i += step;
    }
    Ok(())
  }

  /// `@while cond`
  fn compile_while(
    &mut self,
    cond: &Expr,
    body: &[Statement],
    ctx: &Ctx,
    out: &mut Vec<CssNode>,
    children: &mut Vec<CssChild>,
  ) -> Result<()> {
    let mut guard = 0usize;
    loop {
      let c = self.eval_expr(cond)?;
      if !c.is_truthy() {
        break;
      }
      guard += 1;
      if guard > 100_000 {
        return Err(Error::runtime("@while exceeded 100000 iterations."));
      }
      let sub = self.compile_stmts_same_scope(body, ctx, out)?;
      children.extend(sub);
    }
    Ok(())
  }

  /// `@import` — dynamic inline import; plain-CSS targets pass through
  fn compile_import(
    &mut self,
    targets: &[vass_core::ast::input::statement::ImportTarget],
    ctx: &Ctx,
    out: &mut Vec<CssNode>,
    children: &mut Vec<CssChild>,
  ) -> Result<()> {
    for target in targets {
      if target.plain_css {
        let url = self.interpolate_string(&target.url)?;
        let mut raw = if target.quoted {
          format!("@import \"{}\"", url)
        } else {
          format!("@import {}", url)
        };
        if let Some(media) = &target.media {
          let media = self.interpolate_string(media)?;
          raw.push(' ');
          raw.push_str(&media);
        }
        out.push(CssNode::Raw(raw, 0));
        continue;
      }
      let url = self.interpolate_string(&target.url)?;
      let path = self.resolve_import(&url)?;
      if self.is_current_file(&path) {
        return Err(Error::import(format!(
          "Circular import cycle detected: {}.",
          path.display()
        )));
      }
      let (source, canonical) = self.read_source(&path)?;
      let syntax = vass_parser::detect_syntax(&canonical);
      let sheet = vass_parser::parse(&source, syntax)
        .map_err(|e| e.with_file(canonical.display().to_string()))?;
      self.current_file.push(canonical.clone());
      self.import_nesting += 1;
      // @import splices the file into the importing scope: variables
      // defined there are visible afterwards (reference semantics),
      // so the body must NOT run in a pushed child scope
      let result = self.compile_stmts_root(&sheet.body, ctx, out);
      self.import_nesting -= 1;
      self.current_file.pop();
      let sub = result?;
      children.extend(sub);
    }
    Ok(())
  }

  // ---- expressions -----------------------------------------------------

  fn eval_expr(&mut self, expr: &Expr) -> Result<Value> {
    match expr {
      Expr::Null => Ok(Value::Null),
      Expr::Bool(b) => Ok(Value::Bool(*b)),
      Expr::Number { value, unit } => Ok(Value::Number(Number::new(*value, unit.clone()))),
      Expr::Str { text, quoted } => {
        let value = Value::Str(VassString::new(text.clone(), *quoted));
        if *quoted {
          return Ok(value);
        }
        // `calc(...)` / `clamp(...)` written as plain-CSS functions
        // simplify when every operand resolves to a number
        // (calc(1px + 2px) -> 3px); anything unresolvable (var(),
        // incompatible units, interpolation) stays verbatim.
        simplify_calc_like(&value)
      }
      Expr::Interp { parts, quoted } => {
        let mut out = String::new();
        for part in parts {
          match part {
            StrPart::Text(t) => out.push_str(t),
            StrPart::Expr(e) => {
              let v = self.eval_expr(e)?;
              match v {
                Value::Str(s) => out.push_str(&s.text),
                // null contributes nothing in interpolation
                // (reference: a-#{null}-b is a--b)
                Value::Null => {}
                other => out.push_str(&other.to_css()),
              }
            }
          }
        }
        let value = Value::Str(VassString::new(out, *quoted));
        if matches!(value, Value::Str(ref s) if s.quoted) {
          return Ok(value);
        }
        simplify_calc_like(&value)
      }
      Expr::InterpValue { expr } => {
        let v = self.eval_expr(expr)?;
        Ok(match v {
          Value::Str(s) => Value::Str(VassString::unquoted(s.text)),
          other => Value::Str(VassString::unquoted(other.to_css())),
        })
      }
      Expr::Color { hex } => {
        // Hex literals keep their source text; bare keywords resolve
        // through the named-color table with notation preserved
        let color = if hex.starts_with('#') {
          Color::from_hex(hex)?
        } else {
          Color::from_named(hex)
            .ok_or_else(|| Error::type_error(format!("Unknown color: {}.", hex)))?
        };
        Ok(Value::Color(color))
      }
      Expr::Variable { name, span } => self.env.get(name).cloned().ok_or_else(|| {
        Error::undefined_variable(name.clone())
          .with_help(format!(
            "declare it with `${name}: value;` or `@use` the module that defines it",
            name = name
          ))
          .with_span(*span)
      }),
      Expr::ModuleVar {
        namespace,
        name,
        span,
      } => {
        if !self.env.modules.contains_key(namespace.as_str()) {
          return Err(
            self
              .namespace_error(namespace, Some((name, true)))
              .with_span(*span),
          );
        }
        self.module_variable(namespace, name).ok_or_else(|| {
          self
            .undefined_module_member(namespace, name)
            .with_span(*span)
        })
      }
      Expr::Unary { op, expr } => {
        let v = self.eval_expr(expr)?;
        match (op, &v) {
          (UnaryOp::Neg, Value::Number(n)) => Ok(Value::Number(n.negate())),
          (UnaryOp::Pos, Value::Number(_)) => Ok(v),
          (UnaryOp::Not, other) => Ok(Value::Bool(!other.is_truthy())),
          (UnaryOp::Neg, Value::Null) | (UnaryOp::Pos, Value::Null) => Ok(Value::Null),
          (UnaryOp::Neg, Value::Str(s)) if !s.quoted => {
            Ok(Value::Str(VassString::unquoted(format!("-{}", s.text))))
          }
          _ => Err(Error::type_error(format!(
            "`{}` is not a number.",
            v.to_css()
          ))),
        }
      }
      Expr::Binary {
        op,
        left,
        right,
        allow_slash_div,
      } => self.eval_binary(*op, left, right, *allow_slash_div),
      Expr::List {
        items,
        separator,
        bracketed,
      } => {
        let mut vals = Vec::with_capacity(items.len());
        for item in items {
          vals.push(self.eval_expr(item)?);
        }
        Ok(Value::List(ListValue {
          items: vals,
          sep: *separator,
          bracketed: *bracketed,
        }))
      }
      Expr::Map { entries } => {
        let mut map = VassMap::new();
        for (k, v) in entries {
          let key = self.eval_expr(k)?;
          let val = self.eval_expr(v)?;
          map.set(key, val);
        }
        Ok(Value::Map(map))
      }
      Expr::Paren { expr, .. } => self.eval_expr(expr),
      Expr::Call {
        name, args, span, ..
      } => self.eval_call(name, args, *span),
    }
  }

  fn eval_binary(
    &mut self,
    op: BinaryOp,
    left: &Expr,
    right: &Expr,
    allow_slash_div: bool,
  ) -> Result<Value> {
    // Short-circuit logic
    if op == BinaryOp::And || op == BinaryOp::Or {
      let lv = self.eval_expr(left)?;
      let lt = lv.is_truthy();
      if op == BinaryOp::And {
        return Ok(if lt { self.eval_expr(right)? } else { lv });
      }
      return Ok(if lt { lv } else { self.eval_expr(right)? });
    }

    let lv = self.eval_expr(left)?;

    // String concatenation with `+` / `-` — but two numbers always stay
    // numeric. `+` quoting: a string on the LEFT decides; otherwise a
    // quoted string on the right makes the result quoted
    // (`1 + "a"` -> "1a", `red + "a"` -> "reda", `a + "b"` -> ab).
    // `-` with any string operand joins the CSS forms with `-`, unquoted
    // (`sans - serif` -> sans-serif, `"sans" - "serif"` -> "sans"-"serif",
    // `1 - a` -> 1-a) — reference behavior.
    if matches!(op, BinaryOp::Add | BinaryOp::Sub)
      && (matches!(lv, Value::Str(_)) || expr_may_be_string(right))
    {
      let rv = self.eval_expr(right)?;
      match (&lv, &rv) {
        (Value::Number(_), Value::Number(_)) => {
          // fall through to numeric addition below
        }
        _ if op == BinaryOp::Add => {
          let quoted = match (&lv, &rv) {
            (Value::Str(l), _) => l.quoted,
            (_, Value::Str(r)) => r.quoted,
            _ => false,
          };
          let ls = value_to_concat_text(&lv);
          let rs = value_to_concat_text(&rv);
          return Ok(Value::Str(VassString::new(format!("{}{}", ls, rs), quoted)));
        }
        _ => {
          return Ok(Value::Str(VassString::unquoted(format!(
            "{}-{}",
            lv.to_css(),
            rv.to_css()
          ))));
        }
      }
    }

    let rv = self.eval_expr(right)?;
    // `/` between two plain literals is a CSS separator, not division
    if op == BinaryOp::Div && !allow_slash_div {
      return Ok(Value::Str(VassString::unquoted(format!(
        "{}/{}",
        lv.to_css(),
        rv.to_css()
      ))));
    }
    if op == BinaryOp::Eq {
      return Ok(Value::Bool(values_equal(&lv, &rv)));
    }
    if op == BinaryOp::Ne {
      return Ok(Value::Bool(!values_equal(&lv, &rv)));
    }
    match (&lv, &rv) {
      (Value::Number(a), Value::Number(b)) => {
        // Comparisons
        if matches!(
          op,
          BinaryOp::Lt | BinaryOp::Gt | BinaryOp::Le | BinaryOp::Ge
        ) {
          let ord = a.partial_cmp(b).ok_or_else(|| {
            Error::type_error(format!(
              "Incompatible units for comparison: {} and {}.",
              a.to_css(),
              b.to_css()
            ))
          })?;
          let result = match op {
            BinaryOp::Lt => ord == std::cmp::Ordering::Less,
            BinaryOp::Gt => ord == std::cmp::Ordering::Greater,
            BinaryOp::Le => ord != std::cmp::Ordering::Greater,
            BinaryOp::Ge => ord != std::cmp::Ordering::Less,
            _ => false,
          };
          return Ok(Value::Bool(result));
        }
        // Arithmetic
        let n: Value = match op {
          BinaryOp::Add => Value::Number(a.add(b)?),
          BinaryOp::Sub => Value::Number(a.sub(b)?),
          BinaryOp::Mul => {
            // A product of two unit-bearing numbers cannot be
            // represented — the reference emits calc()
            match a.mul(b) {
              Ok(n) => Value::Number(n),
              Err(_e) if a.unit.is_some() && b.unit.is_some() => Value::Str(VassString::unquoted(
                format!("calc({} * {})", a.to_css(), b.to_css()),
              )),
              Err(e) => return Err(e),
            }
          }
          BinaryOp::Mod => Value::Number(a.modulo(b)?),
          BinaryOp::Div => {
            // Division that cannot cancel into a plain number is
            // emitted as calc(), with operands normalized the way
            // the reference compiler does (calc(2mm / 1m))
            match a.div(b) {
              Ok(n) => Value::Number(n),
              Err(_e) if b.unit.is_some() || b.value == 0.0 => {
                Value::Str(VassString::unquoted(a.div_calc(b)))
              }
              Err(e) => return Err(e),
            }
          }
          _ => {
            return Err(Error::type_error(format!(
              "Unsupported operands for `{}`.",
              op.as_str()
            )));
          }
        };
        Ok(n)
      }
      // Legacy color arithmetic (component-wise), matching the
      // reference implementation's deprecated-but-supported behavior
      (Value::Color(a), Value::Color(b)) => match op {
        BinaryOp::Add => Ok(Value::Color(color_component_op(a, b, false))),
        BinaryOp::Sub => Ok(Value::Color(color_component_op(a, b, true))),
        _ => Err(Error::type_error(format!(
          "Undefined operation \"{} {}\".",
          a.to_css(),
          op.as_str()
        ))),
      },
      (Value::Color(a), Value::Number(b)) => match op {
        BinaryOp::Mul => Ok(Value::Color(scale_color_channels(a, b.value))),
        BinaryOp::Div => Ok(Value::Color(scale_color_channels(a, 1.0 / b.value))),
        _ => Err(Error::type_error(format!(
          "Undefined operation \"{} {} {}\".",
          a.to_css(),
          op.as_str(),
          b.to_css()
        ))),
      },
      (Value::Number(a), Value::Color(b)) => match op {
        BinaryOp::Mul => Ok(Value::Color(scale_color_channels(b, a.value))),
        _ => Err(Error::type_error(format!(
          "Undefined operation \"{} {} {}\".",
          a.to_css(),
          op.as_str(),
          b.to_css()
        ))),
      },
      // `/` that reached the division context with a non-numeric
      // operand (e.g. quoted string) is a literal slash-separated
      // value: `12px / "Inter"` → 12px/"Inter" (reference behavior)
      _ if op == BinaryOp::Div => Ok(Value::Str(VassString::unquoted(format!(
        "{}/{}",
        lv.to_css(),
        rv.to_css()
      )))),
      _ => Err(Error::type_error(format!(
        "Unsupported operands for `{}`: `{}` and `{}`.",
        op.as_str(),
        lv.to_css(),
        rv.to_css()
      ))),
    }
  }

  /// Function call: evaluator builtins, user functions, module functions,
  /// global builtins, then plain-CSS passthrough
  fn eval_call(
    &mut self,
    name: &str,
    args: &[CallArg],
    span: vass_core::token::Span,
  ) -> Result<Value> {
    // Namespaced: `math.div(...)`
    if let Some((ns, member)) = name.split_once('.') {
      return self.eval_module_call(ns, member, args, span);
    }

    // Evaluator-state builtins
    match name {
      "if" => {
        let cond = self.eval_expr(&args.first().map(|a| a.expr.clone()).unwrap_or(Expr::Null))?;
        if cond.is_truthy() {
          return self.eval_expr(&args.get(1).map(|a| a.expr.clone()).unwrap_or(Expr::Null));
        }
        return self.eval_expr(&args.get(2).map(|a| a.expr.clone()).unwrap_or(Expr::Null));
      }
      "variable-exists" => {
        let n = self.eval_name_arg(args)?;
        return Ok(Value::Bool(self.env.is_defined(&n)));
      }
      "global-variable-exists" => {
        let n = self.eval_name_arg(args)?;
        return Ok(Value::Bool(self.env.is_defined_globally(&n)));
      }
      "function-exists" => {
        let n = self.eval_name_arg(args)?;
        return Ok(Value::Bool(
          self.env.has_function(&n) || builtins_known_function(&n),
        ));
      }
      "mixin-exists" => {
        let n = self.eval_name_arg(args)?;
        return Ok(Value::Bool(self.env.has_mixin(&n)));
      }
      "content-exists" => {
        return Ok(Value::Bool(
          self.content.last().map(|b| b.is_some()).unwrap_or(false),
        ));
      }
      "feature-exists" => {
        let n = self.eval_name_arg(args)?;
        return Ok(Value::Bool(matches!(
          n.as_str(),
          "global-builtin-functions" | "module-system" | "at-error" | "units-level-3"
        )));
      }
      "get-function" => {
        let n = self.eval_name_arg(args)?;
        let user = self.env.has_function(&n);
        if !user && !builtins_known_function(&n) {
          let css_flag = args
            .get(1)
            .map(|a| a.expr.clone())
            .map(|e| self.eval_expr(&e))
            .transpose()?
            .map(|v| matches!(v, Value::Bool(true)))
            .unwrap_or(false);
          if !css_flag {
            return Err(Error::undefined_function(n).with_span(span));
          }
        }
        return Ok(Value::Function(FunctionRef {
          name: n,
          user_defined: user,
        }));
      }
      "call" => {
        // call($fn, args...): first argument is a function value
        // (or name), the rest are forwarded
        let (evaluated, _, _) = self.eval_args(args)?;
        if evaluated.is_empty() {
          return Err(Error::type_error("call() needs a function."));
        }
        let fname = match &evaluated[0] {
          Value::Function(f) => f.name.clone(),
          Value::Str(s) => s.text.clone(),
          other => {
            return Err(Error::type_error(format!(
              "call: not a function: {}.",
              other.inspect()
            )));
          }
        };
        let rest: Vec<CallArg> = args.iter().skip(1).cloned().collect();
        return self.eval_call(&fname, &rest, span);
      }
      _ => {}
    }

    // User functions
    if let Some(f) = self.env.get_function(name) {
      let bound = self.bind_call_args(&f, args, name)?;
      return self.call_function(&f, bound, span);
    }

    // Global builtins (rgb/rgba/hsl/hsla notation preservation now
    // happens inside the builtins themselves, covering comma, space and
    // two-arg forms uniformly)
    let (evaluated, named, _) = self.eval_args(args)?;
    let args_obj = builtins::Args {
      positional: evaluated.iter().collect(),
      named: named.iter().map(|(k, v)| (k.clone(), v)).collect(),
    };
    // `rgb(var(--accent), 0.5)` and friends: when a color-function
    // argument is a special CSS value (`var()`, `env()`, `calc()`,
    // `clamp()`) the color cannot be resolved statically, so the
    // reference compiler emits the call verbatim instead of erroring.
    // Space-separated modern-syntax arguments are flattened into
    // comma-separated parts (`rgb(var(--x) 255 0)` ->
    // `rgb(var(--x), 255, 0)`), matching the reference.
    if matches!(name, "rgb" | "rgba" | "hsl" | "hsla")
      && named.is_empty()
      && evaluated.iter().any(contains_special_css_fn)
    {
      let mut parts: Vec<String> = Vec::new();
      for v in &evaluated {
        match v {
          Value::List(l) if l.sep == Separator::Space => {
            for item in &l.items {
              parts.push(item.to_css());
            }
          }
          other => parts.push(other.to_css()),
        }
      }
      return Ok(builtins::unquoted(format!(
        "{}({})",
        name,
        parts.join(", ")
      )));
    }
    if let Some(result) = builtins::call_global(name, args_obj) {
      return result;
    }

    // Modern CSS Color 4 function forms become first-class colors
    if matches!(
      name.to_ascii_lowercase().as_str(),
      "oklch" | "oklab" | "lab" | "lch" | "hwb" | "color"
    ) && named.is_empty()
      && let Some(c) = try_modern_color(name, &evaluated)
    {
      return Ok(Value::Color(c));
    }

    // Plain CSS passthrough: re-serialize evaluated arguments
    self.plain_css_call(name, &evaluated, &named, span)
  }

  /// A namespaced call: user module first, then builtin module
  fn eval_module_call(
    &mut self,
    ns: &str,
    member: &str,
    args: &[CallArg],
    span: vass_core::token::Span,
  ) -> Result<Value> {
    // dart-sass strictness: namespace access requires an explicit `@use`
    if !self.env.modules.contains_key(ns) {
      return Err(
        self
          .namespace_error(ns, Some((member, false)))
          .with_span(span),
      );
    }
    if let Some(module) = self.env.modules.get(ns).cloned()
      && let Some(f) = module.functions.get(member).cloned()
    {
      let bound = self.bind_call_args(&f, args, member)?;
      return self.call_function(&f, bound, span);
    }
    let (evaluated, named, _) = self.eval_args(args)?;
    let args_obj = builtins::Args {
      positional: evaluated.iter().collect(),
      named: named.iter().map(|(k, v)| (k.clone(), v)).collect(),
    };
    // Dispatch by the REAL module name, not the user's alias
    let real_ns = self
      .builtin_ns
      .get(ns)
      .cloned()
      .unwrap_or_else(|| ns.to_string());
    if let Some(result) = builtins::call_module(&real_ns, member, args_obj) {
      return result;
    }
    if real_ns == "meta" {
      return self.meta_call(member, args, span);
    }
    if builtins::module_function_names(&real_ns).contains(&member) {
      return Err(Error::undefined_function(format!("{}.{}", ns, member)).with_span(span));
    }
    let full_name = format!("{}.{}", ns, member);
    self.plain_css_call(&full_name, &evaluated, &named, span)
  }

  /// meta.call / meta.get-function / module introspection
  fn meta_call(
    &mut self,
    member: &str,
    args: &[CallArg],
    span: vass_core::token::Span,
  ) -> Result<Value> {
    match member {
      "get-function" => {
        let n = self.eval_name_arg(args)?;
        let user = self.env.has_function(&n);
        Ok(Value::Function(FunctionRef {
          name: n,
          user_defined: user,
        }))
      }
      "get-mixin" => {
        let n = self.eval_name_arg(args)?;
        if !self.env.has_mixin(&n) {
          return Err(Error::undefined_mixin(format!(
            "meta.get-mixin: no mixin named `{}`.",
            n
          )));
        }
        Ok(Value::Mixin(n))
      }
      "call" => {
        let (evaluated, _, _) = self.eval_args(args)?;
        if evaluated.is_empty() {
          return Err(Error::type_error("meta.call() needs a function."));
        }
        let fname = match &evaluated[0] {
          Value::Function(f) => f.name.clone(),
          Value::Str(s) => s.text.clone(),
          other => {
            return Err(Error::type_error(format!(
              "meta.call: not a function: {}.",
              other.inspect()
            )));
          }
        };
        let rest: Vec<CallArg> = args.iter().skip(1).cloned().collect();
        self.eval_call(&fname, &rest, span)
      }
      "module-variables" | "module-functions" => {
        let n = self.eval_name_arg(args)?;
        let module = self.env.modules.get(&n).cloned();
        match module {
          Some(m) => {
            let entries: Vec<(Value, Value)> = if member == "module-variables" {
              m.variables
                .iter()
                .map(|(k, v)| (Value::Str(VassString::unquoted(k.clone())), v.clone()))
                .collect()
            } else {
              m.functions
                .keys()
                .map(|k| {
                  (
                    Value::Str(VassString::unquoted(k.clone())),
                    Value::Function(FunctionRef {
                      name: k.clone(),
                      user_defined: true,
                    }),
                  )
                })
                .collect()
            };
            Ok(Value::Map(VassMap { entries }))
          }
          None => Err(Error::import(format!("No module named `{}`.", n))),
        }
      }
      "variable-exists" => {
        let n = self.eval_name_arg(args)?;
        Ok(Value::Bool(self.env.is_defined(&n)))
      }
      "global-variable-exists" => {
        let n = self.eval_name_arg(args)?;
        Ok(Value::Bool(self.env.is_defined_globally(&n)))
      }
      "function-exists" => {
        let n = self.eval_name_arg(args)?;
        Ok(Value::Bool(
          self.env.has_function(&n) || builtins_known_function(&n),
        ))
      }
      "mixin-exists" => {
        let n = self.eval_name_arg(args)?;
        Ok(Value::Bool(self.env.has_mixin(&n)))
      }
      "content-exists" => {
        if self.content.is_empty() {
          return Err(
            Error::runtime("meta.content-exists() may only be called within a mixin.")
              .with_span(span),
          );
        }
        Ok(Value::Bool(self.content.last().unwrap().is_some()))
      }
      "feature-exists" => {
        let n = self.eval_name_arg(args)?;
        // supported reference features
        Ok(Value::Bool(matches!(
          n.as_str(),
          "at-error"
            | "global-variable-shadowing"
            | "extend-selector-pseudoclass"
            | "units-level-3"
            | "custom-function"
        )))
      }
      other => Err(Error::undefined_function(format!("meta.{}", other)).with_span(span)),
    }
  }

  /// Serialize an unknown function call back to plain CSS
  fn plain_css_call(
    &mut self,
    name: &str,
    evaluated: &[Value],
    named: &[(String, Value)],
    span: vass_core::token::Span,
  ) -> Result<Value> {
    if !named.is_empty() {
      return Err(
        Error::type_error(format!(
          "Plain CSS function `{}` does not accept named arguments.",
          name
        ))
        .with_span(span),
      );
    }
    if evaluated.iter().any(|v| matches!(v, Value::Map(_))) {
      return Err(
        Error::type_error(format!(
          "Plain CSS function `{}` cannot take a map argument.",
          name
        ))
        .with_span(span),
      );
    }
    let inner = evaluated
      .iter()
      .map(|v| match v {
        Value::List(l) if l.sep == Separator::Comma => l.to_css(),
        other => other.to_css(),
      })
      .collect::<Vec<_>>()
      .join(", ");
    Ok(Value::Str(VassString::unquoted(format!(
      "{}({})",
      name, inner
    ))))
  }

  /// Evaluate call args: positional values, named values, rest spreads
  fn eval_args(&mut self, args: &[CallArg]) -> Result<EvaluatedArgs> {
    let mut positional = Vec::new();
    let mut named = Vec::new();
    for arg in args {
      let v = self.eval_expr(&arg.expr)?;
      if arg.rest {
        match v {
          Value::List(l) => {
            for item in l.items {
              positional.push(item);
            }
          }
          Value::ArgList(a) => {
            for item in a.list.items {
              positional.push(item);
            }
            for (k, val) in a.named {
              if let Value::Str(s) = &k {
                named.push((s.text.clone(), val));
              }
            }
          }
          Value::Map(m) => {
            for (k, val) in m.entries {
              if let Value::Str(s) = &k {
                named.push((s.text.clone(), val));
              } else {
                positional.push(Value::List(ListValue::new(vec![k, val], Separator::Space)));
              }
            }
          }
          other => positional.push(other),
        }
      } else if let Some(n) = &arg.name {
        named.push((n.clone(), v));
      } else {
        positional.push(v);
      }
    }
    Ok((positional, named, None))
  }

  fn eval_name_arg(&mut self, args: &[CallArg]) -> Result<String> {
    let v = match args.first() {
      Some(a) => self.eval_expr(&a.expr)?,
      None => return Err(Error::type_error("Expected a name argument.")),
    };
    Ok(match v {
      Value::Str(s) => s.text,
      other => other.to_css(),
    })
  }

  // ---- function/mixin invocation ----------------------------------------

  fn resolve_callable(
    &mut self,
    name: &str,
    mixin: bool,
    context: &str,
  ) -> Result<Rc<UserCallable>> {
    // Namespaced member: `module.name`
    if let Some((ns, member)) = name.split_once('.') {
      if !self.env.modules.contains_key(ns) {
        return Err(self.namespace_error(ns, Some((member, false))));
      }
      if let Some(module) = self.env.modules.get(ns).cloned() {
        let found = if mixin {
          module.mixins.get(member).cloned()
        } else {
          module.functions.get(member).cloned()
        };
        if let Some(f) = found {
          return Ok(f);
        }
      }
    }
    let found = if mixin {
      self.env.get_mixin(name)
    } else {
      self.env.get_function(name)
    };
    found.ok_or_else(|| {
            if mixin {
                Error::undefined_mixin(format!("{}: no mixin named `{}`.", context, name))
                    .with_help(format!(
                        "define it first with `@mixin {name} {{ ... }}` (mixins must be declared before use), or check the spelling",
                        name = name
                    ))
            } else {
                Error::undefined_function(format!("{}: no function named `{}`.", context, name))
                    .with_help(format!(
                        "define it first with `@function {name}(...) {{ ... }}`, or check the spelling",
                        name = name
                    ))
            }
        })
  }

  /// Bind call arguments to parameters; defaults are deferred to the
  /// callee scope via `pending_defaults`
  fn bind_call_args(
    &mut self,
    callable: &UserCallable,
    args: &[CallArg],
    name: &str,
  ) -> Result<Vec<(String, Value)>> {
    let mut positional: Vec<Value> = Vec::new();
    let mut named: Vec<(String, Value)> = Vec::new();
    for arg in args {
      let v = self.eval_expr(&arg.expr)?;
      if arg.rest {
        match v {
          Value::List(l) => {
            for item in l.items {
              positional.push(item);
            }
          }
          Value::ArgList(a) => {
            for item in a.list.items {
              positional.push(item);
            }
            for (k, val) in a.named {
              if let Value::Str(s) = &k {
                named.push((s.text.clone(), val));
              }
            }
          }
          Value::Map(m) => {
            for (k, val) in m.entries {
              if let Value::Str(s) = &k {
                named.push((s.text.clone(), val));
              } else {
                positional.push(Value::List(ListValue::new(vec![k, val], Separator::Space)));
              }
            }
          }
          other => positional.push(other),
        }
      } else if let Some(n) = &arg.name {
        named.push((n.clone(), v));
      } else {
        positional.push(v);
      }
    }
    let mut rest_list: Vec<Value> = Vec::new();
    let mut rest_named: Vec<(Value, Value)> = Vec::new();
    // Named arguments bind to their parameters first; positional
    // arguments then fill the remaining unfilled params in order
    let mut slots: Vec<Option<Value>> = vec![None; callable.params.len()];
    let mut rest_slot: Option<usize> = None;
    for (i, param) in callable.params.iter().enumerate() {
      if param.rest {
        rest_slot = Some(i);
        break;
      }
    }
    for (k, v) in named {
      let pos = callable.params.iter().position(|p| p.name == k);
      match pos {
        Some(i) if slots[i].is_none() => slots[i] = Some(v),
        Some(_) => {
          return Err(Error::type_error(format!(
            "Argument `${}` passed twice to {}().",
            k, name
          )));
        }
        None if rest_slot.is_some() => {
          rest_named.push((Value::Str(VassString::unquoted(k)), v));
        }
        None => {
          return Err(Error::type_error(format!(
            "No argument named `${}` for {}().",
            k, name
          )));
        }
      }
    }
    let mut pi = 0usize;
    for (i, param) in callable.params.iter().enumerate() {
      if param.rest {
        continue;
      }
      if slots[i].is_none() && pi < positional.len() {
        slots[i] = Some(positional[pi].clone());
        pi += 1;
      }
    }
    if pi < positional.len() && rest_slot.is_none() {
      return Err(Error::type_error(format!(
        "Only {} argument(s) allowed, but {} were passed to {}().",
        callable.params.len(),
        positional.len(),
        name
      )));
    }
    let mut bound: Vec<(String, Value)> = Vec::new();
    for (i, param) in callable.params.iter().enumerate() {
      if param.rest {
        while pi < positional.len() {
          rest_list.push(positional[pi].clone());
          pi += 1;
        }
        let al = ArgListValue {
          list: ListValue::new(std::mem::take(&mut rest_list), Separator::Comma),
          named: std::mem::take(&mut rest_named),
        };
        bound.push((param.name.clone(), Value::ArgList(Box::new(al))));
        continue;
      }
      match slots[i].take() {
        Some(v) => bound.push((param.name.clone(), v)),
        None => match &param.default {
          Some(_) => {
            // Defer default evaluation to the callee scope
            self.pending_defaults.push(param.clone());
            bound.push((param.name.clone(), Value::Null));
          }
          None => {
            return Err(Error::type_error(format!(
              "Missing argument `${}` for {}().",
              param.name, name
            )));
          }
        },
      }
    }
    Ok(bound)
  }

  /// Call a user function and return its `@return` value
  fn call_function(
    &mut self,
    f: &UserCallable,
    bound: Vec<(String, Value)>,
    span: vass_core::token::Span,
  ) -> Result<Value> {
    self.depth += 1;
    if self.depth > MAX_CALL_DEPTH {
      self.depth = 0;
      return Err(
        Error::runtime(format!(
          "Maximum call stack size exceeded calling {}().",
          f.name
        ))
        .with_span(span),
      );
    }
    let body = f.body.clone();
    // Restore the defining scope(s) beneath the call scope
    self.env.push_scopes(&f.env_snapshot);
    self.env.push_scope();
    for (k, v) in bound {
      self.env.define_local(&k, v);
    }
    let result = self.apply_pending_defaults();
    if let Err(e) = result {
      self.env.pop_scope();
      self.env.pop_scopes(f.env_snapshot.len());
      self.depth -= 1;
      return Err(e);
    }
    let saved_return = self.return_value.take();
    let exec = self.exec_function_body(&body);
    let returned = self.return_value.take();
    self.return_value = saved_return;
    self.env.pop_scope();
    self.env.pop_scopes(f.env_snapshot.len());
    self.depth -= 1;
    exec?;
    returned.ok_or_else(|| Error::syntax(format!("Function {} finished without @return.", f.name)))
  }

  /// Execute function-body statements (no CSS emission): variables,
  /// control flow, and `@return` capture
  fn exec_function_body(&mut self, stmts: &[Statement]) -> Result<()> {
    for stmt in stmts {
      if self.return_value.is_some() {
        return Ok(());
      }
      match stmt {
        Statement::Return { value } => {
          let v = self.eval_expr(value)?;
          self.return_value = Some(v);
          return Ok(());
        }
        Statement::VariableDecl {
          name,
          value,
          guarded,
          global,
        } => {
          if *guarded && self.env.is_defined(name) {
            continue;
          }
          let v = self.eval_expr(value)?;
          self.env.set(name, v, *global);
        }
        Statement::If(chain) => {
          for (cond, body) in &chain.clauses {
            let c = self.eval_expr(cond)?;
            if c.is_truthy() {
              self.exec_function_body(body)?;
              return Ok(());
            }
          }
          if let Some(else_body) = &chain.else_body {
            self.exec_function_body(else_body)?;
          }
        }
        Statement::Each { vars, list, body } => {
          let lv = self.eval_expr(list)?;
          let (items, _, _) = builtins::to_list(&lv);
          for item in items {
            let tuple = builtins::to_list(&item).0;
            self.env.push_scope();
            for (i, var) in vars.iter().enumerate() {
              let v = if vars.len() == 1 {
                item.clone()
              } else {
                tuple.get(i).cloned().unwrap_or(Value::Null)
              };
              self.env.define_local(var, v);
            }
            let r = self.exec_function_body(body);
            self.env.pop_scope();
            r?;
            if self.return_value.is_some() {
              return Ok(());
            }
          }
        }
        Statement::For {
          var,
          from,
          to,
          inclusive,
          body,
        } => {
          let f = self.eval_expr(from)?;
          let t = self.eval_expr(to)?;
          let fv = match &f {
            Value::Number(n) => n.value.round() as i64,
            _ => return Err(Error::type_error("@for: $from must be a number.")),
          };
          let tv = match &t {
            Value::Number(n) => n.value.round() as i64,
            _ => return Err(Error::type_error("@for: $to must be a number.")),
          };
          let step = if fv <= tv { 1 } else { -1 };
          let end = if *inclusive { tv } else { tv - step };
          let mut i = fv;
          while (step > 0 && i <= end) || (step < 0 && i >= end) {
            self.env.push_scope();
            self
              .env
              .define_local(var, Value::Number(Number::unitless(i as f64)));
            let r = self.exec_function_body(body);
            self.env.pop_scope();
            r?;
            if self.return_value.is_some() {
              return Ok(());
            }
            i += step;
          }
        }
        Statement::While { cond, body } => {
          let mut guard = 0usize;
          loop {
            let c = self.eval_expr(cond)?;
            if !c.is_truthy() {
              break;
            }
            guard += 1;
            if guard > 100_000 {
              return Err(Error::runtime("@while exceeded 100000 iterations."));
            }
            self.exec_function_body(body)?;
            if self.return_value.is_some() {
              return Ok(());
            }
          }
        }
        Statement::Error { value } => {
          let v = self.eval_expr(value)?;
          return Err(Error::runtime(v.inspect()));
        }
        Statement::Comment { .. } => {}
        other => {
          return Err(Error::syntax(format!(
            "This construct is not allowed inside a function: {:?}.",
            match other {
              Statement::Declaration { prop, .. } => format!("declaration `{}`", prop),
              Statement::StyleRule { selector, .. } => {
                format!("style rule `{}`", selector)
              }
              _ => "statement".to_string(),
            }
          )));
        }
      }
    }
    Ok(())
  }

  /// Invoke a mixin, compiling its body into the current context
  fn invoke_mixin(
    &mut self,
    m: &UserCallable,
    bound: Vec<(String, Value)>,
    content: Option<ContentBlock>,
    ctx: &Ctx,
    out: &mut Vec<CssNode>,
    children: &mut Vec<CssChild>,
  ) -> Result<()> {
    self.depth += 1;
    if self.depth > MAX_CALL_DEPTH {
      self.depth = 0;
      return Err(Error::runtime(format!(
        "Maximum call stack size exceeded calling mixin {}.",
        m.name
      )));
    }
    let body = m.body.clone();
    // Restore the defining scope(s) beneath the call scope
    self.env.push_scopes(&m.env_snapshot);
    self.env.push_scope();
    for (k, v) in bound {
      self.env.define_local(&k, v);
    }
    let result = self.apply_pending_defaults();
    if let Err(e) = result {
      self.env.pop_scope();
      self.env.pop_scopes(m.env_snapshot.len());
      self.depth -= 1;
      return Err(e);
    }
    self.content.push(content);
    let result = self.compile_stmts_inner(&body, ctx, out, children);
    self.content.pop();
    self.env.pop_scope();
    self.env.pop_scopes(m.env_snapshot.len());
    self.depth -= 1;
    result
  }

  /// Evaluate deferred parameter defaults in the callee scope
  fn apply_pending_defaults(&mut self) -> Result<()> {
    let pending = std::mem::take(&mut self.pending_defaults);
    for param in pending {
      if self.env.is_defined(&param.name) && !matches!(self.env.get(&param.name), Some(Value::Null))
      {
        continue;
      }
      if let Some(default) = &param.default {
        let v = self.eval_expr(default)?;
        self.env.define_local(&param.name, v);
      }
    }
    Ok(())
  }

  // ---- interpolation ---------------------------------------------------

  /// Resolve `#{...}` interpolation inside raw text
  fn interpolate_string(&mut self, text: &str) -> Result<String> {
    if !text.contains("#{") {
      return Ok(text.to_string());
    }
    let expr = vass_parser::parse_interp_text(text)
      .map_err(|e| Error::syntax(format!("Invalid interpolation: {}", e.message)))?;
    let v = self.eval_expr(&expr)?;
    Ok(match v {
      Value::Str(s) => s.text,
      // null interpolates to nothing (reference: a-#{null}-b -> a--b)
      Value::Null => String::new(),
      other => other.to_css(),
    })
  }

  /// Evaluate a `@media` prelude the way the reference compiler does:
  /// interpolation resolves first, then each comma-separated query is
  /// structurally evaluated — the media type passes through, feature
  /// values (`(min-width: $v)`, `(width: 100px + 5)`) are evaluated as
  /// expressions — and the list is re-serialized in normalized form
  /// (`(min-width:960px   )` -> `(min-width: 960px)`, `AND` -> `and`).
  fn eval_media_prelude(&mut self, params: &str) -> Result<String> {
    let text = self.interpolate_string(params)?;
    let mut queries: Vec<String> = Vec::new();
    for seg in split_media_top_level(&text, ',') {
      let seg = seg.trim();
      if seg.is_empty() {
        return Err(Error::syntax("Expected media query.".to_string()));
      }
      queries.push(self.eval_media_query(seg)?);
    }
    Ok(queries.join(", "))
  }

  /// Evaluate one comma-separated media query
  fn eval_media_query(&mut self, seg: &str) -> Result<String> {
    let parts = split_media_on_keywords(seg);
    let mut type_part = String::new();
    let mut saw_type = false;
    let mut conds: Vec<(&'static str, String)> = Vec::new();
    for (kw, part) in parts {
      let p = part.trim();
      if p.starts_with('(') {
        conds.push((kw.unwrap_or("and"), self.eval_media_condition(p)?));
        continue;
      }
      // Media type, optionally with a `not`/`only` modifier
      if !conds.is_empty() {
        // A type may not follow conditions (reference:
        // "expected media condition in parentheses.")
        return Err(Error::syntax(
          "expected media condition in parentheses.".to_string(),
        ));
      }
      if saw_type {
        return Err(Error::syntax("Expected media query.".to_string()));
      }
      if p.contains('$') {
        // Bare variables are not allowed in the type position
        // (reference: `@media $v` -> "Expected identifier.")
        return Err(Error::syntax("Expected identifier.".to_string()));
      }
      let words: Vec<&str> = p.split_whitespace().collect();
      if words.len() > 2 {
        return Err(Error::syntax("Expected identifier.".to_string()));
      }
      type_part = words.join(" ");
      saw_type = true;
    }
    if !saw_type && conds.is_empty() {
      return Err(Error::syntax("Expected media query.".to_string()));
    }
    let mut out = type_part;
    for (kw, cond) in conds {
      // The keyword joins a condition to what came before it; the
      // very first condition of a typeless query has no joiner
      if !out.is_empty() {
        out.push(' ');
        out.push_str(kw);
        out.push(' ');
      }
      out.push_str(&cond);
    }
    Ok(out)
  }

  /// Evaluate a single parenthesized media condition: `(name: expr)` or
  /// `(expr)`. The value supports variables, interpolation (already
  /// resolved) and arithmetic, like the reference compiler.
  fn eval_media_condition(&mut self, part: &str) -> Result<String> {
    let (inner, rest) = match_media_parens(part)
      .ok_or_else(|| Error::syntax("expected media condition in parentheses.".to_string()))?;
    if !rest.trim().is_empty() {
      return Err(Error::syntax(
        "expected media condition in parentheses.".to_string(),
      ));
    }
    let inner = inner.trim();
    if inner.is_empty() {
      return Err(Error::syntax(
        "expected media condition in parentheses.".to_string(),
      ));
    }
    match find_top_level_char(inner, ':') {
      Some(ci) => {
        let name = inner[..ci].trim();
        let value_src = inner[ci + 1..].trim();
        if name.is_empty() || name.contains('$') || name.split_whitespace().count() != 1 {
          return Err(Error::syntax("Expected identifier.".to_string()));
        }
        if value_src.is_empty() {
          return Err(Error::syntax(
            "expected media condition in parentheses.".to_string(),
          ));
        }
        let v = self.eval_media_expr(value_src)?;
        Ok(format!("({}: {})", name, v.to_css()))
      }
      None => {
        let v = self.eval_media_expr(inner)?;
        Ok(format!("({})", v.to_css()))
      }
    }
  }

  fn eval_media_expr(&mut self, src: &str) -> Result<Value> {
    let expr =
      vass_parser::parse_expr_snippet(src).map_err(|e| Error::syntax(format!("{}", e.message)))?;
    self.eval_expr(&expr)
  }

  // ---- modules -----------------------------------------------------------

  /// Anchor a module-load failure at the `@use`/`@forward` rule that
  /// caused it (dart-sass underlines the whole rule). Inner errors that
  /// already carry a precise location — e.g. a parse error inside the
  /// loaded module — keep their own span and file.
  fn use_rule_context(&self, e: Error, rule: &UseRule) -> Error {
    let e = e.with_span_if_none(rule.span);
    match self.current_file.last() {
      Some(f) => e.with_file_if_none(f.display().to_string()),
      None => e,
    }
  }

  /// The "no module with namespace" error, enriched with a DX hint:
  /// * a loaded namespace differing only by CASE -> rename suggestion
  ///   (+ quick fix) — the classic `@use 'base' as Base` mismatch
  /// * nothing loaded and no `as *` merge -> how to add the `@use`
  ///   (+ insert quick fix at the top of the file)
  /// * other namespaces loaded -> list them
  fn namespace_error(&self, ns: &str, member_fix: Option<(&str, bool)>) -> Error {
    let err = Error::new(
      vass_core::error::ErrorKind::Runtime,
      format!("There is no module with namespace \"{}\".", ns),
    );
    let mut loaded: Vec<&str> = self.env.modules.keys().map(|s| s.as_str()).collect();
    loaded.sort();

    // Case-insensitive hit: namespaces are case-sensitive, suggest the
    // exact spelling (and offer to rewrite the access).
    if let Some(actual) = loaded.iter().find(|k| k.eq_ignore_ascii_case(ns)) {
      let actual = (*actual).to_string();
      let mut e = err.with_help(format!(
                "a module is loaded with namespace `{actual}` - namespaces are case-sensitive; write `{actual}.*` instead of `{ns}.*`"
            ));
      if let Some((member, is_var)) = member_fix {
        let dollar = if is_var { "$" } else { "" };
        e = e.with_fix(
          format!("Use namespace `{actual}`"),
          FixKind::Replace(format!("{actual}.{dollar}{member}")),
        );
      }
      return e;
    }

    // `as *` merges members into the scope — no namespace exists, so
    // the fix is to DROP the prefix, not to add a @use.
    if self.env.merged_star {
      return err.with_help(
                "a module was loaded with `as *` - its members are available WITHOUT a namespace prefix (`$primary-color`, not `base.$primary-color`)"
                    .to_string(),
            );
    }

    let url = if builtins::is_builtin_module(ns) {
      format!("vass:{ns}")
    } else {
      ns.to_string()
    };

    if loaded.is_empty() {
      return err
                .with_help(format!(
                    "this file loads no modules. Add `@use '{url}';` at the TOP of the file - the namespace comes from the file name (`_base.vcss` gives namespace `base`)"
                ))
                .with_fix(
                    format!("Add @use '{url}';"),
                    FixKind::Insert {
                        line: 1,
                        column: 1,
                        text: format!("@use '{url}';\n"),
                    },
                );
    }

    err.with_help(format!(
      "loaded namespaces: {}. Add `@use '{url}';` to load `{ns}`.",
      loaded.join(", ")
    ))
  }

  /// E0004 for `ns.$member` when the namespace exists but the member
  /// doesn't: lists what the module DOES define and offers the closest
  /// spelling as a quick fix.
  fn undefined_module_member(&self, ns: &str, name: &str) -> Error {
    let err = Error::undefined_variable(format!("{}.{}", ns, name));
    let Some(module) = self.env.modules.get(ns) else {
      return err;
    };
    if name.starts_with('-') {
      return err.with_help(format!(
                "members starting with `-` are private to module `{ns}` and can't be accessed through the namespace"
            ));
    }
    let mut vars: Vec<&str> = module.variables.keys().map(|s| s.as_str()).collect();
    vars.sort();
    if vars.is_empty() {
      return err.with_help(format!(
                "module `{ns}` defines no variables - check the module's file, or `@use` the module that does"
            ));
    }
    let shown: Vec<String> = vars.iter().take(6).map(|v| format!("${v}")).collect();
    let more = if vars.len() > 6 {
      format!(" (and {} more)", vars.len() - 6)
    } else {
      String::new()
    };
    match closest_match(name, &vars) {
      Some(close) => err
        .with_help(format!(
          "module `{ns}` defines: {list}{more} - did you mean `${close}`?",
          list = shown.join(", ")
        ))
        .with_fix(
          format!("Use `${close}`"),
          FixKind::Replace(format!("{ns}.${close}")),
        ),
      None => err.with_help(format!(
        "module `{ns}` defines: {list}{more}",
        list = shown.join(", ")
      )),
    }
  }

  fn module_variable(&mut self, ns: &str, name: &str) -> Option<Value> {
    if let Some(module) = self.env.modules.get(ns) {
      // `-`-prefixed members are PRIVATE: they exist in the module but
      // are invisible through a namespace (reference behaviour)
      if name.starts_with('-') {
        return None;
      }
      if let Some(v) = module.variables.get(name) {
        return Some(v.clone());
      }
    }
    // aliases of built-in modules expose their variables too ($pi, ...)
    let real = self.builtin_ns.get(ns)?;
    builtins::module_variable(real, name)
  }

  /// Resolve a module URL to a canonical path.
  ///
  /// Resolution follows dart-sass (probed against dart-sass 1.107):
  /// - relative to the importing file's directory, then `-I` load paths
  /// - an explicit-extension URL (`base.vcss`) also matches its
  ///   `_`-partial variant (`_base.vcss`)
  /// - an extensionless URL tries, per extension (vass, vcss, scss, css):
  ///   plain file, then `_`-partial, then `name/index`, `name/_index`
  /// - a plain + partial pair with the SAME extension is an ambiguity
  ///   error ("It's not clear which file to import.")
  /// - a failed lookup lists every searched location plus a hint when a
  ///   same-named file exists outside the searched directories
  fn resolve_import(&self, url: &str) -> Result<PathBuf> {
    if url.starts_with("vass:") {
      return Err(Error::import(format!(
        "Built-in module `{}` does not need loading.",
        url
      )));
    }
    let base = self
      .current_file
      .last()
      .and_then(|p| p.parent().map(|d| d.to_path_buf()))
      .unwrap_or_else(|| PathBuf::from("."));
    let mut searched: Vec<String> = Vec::new();
    let dirs = std::iter::once(base.clone()).chain(self.options.load_paths.iter().cloned());
    for dir in dirs {
      if let Some(hit) = probe_candidates(&dir.join(url), &mut searched)? {
        return Ok(canonicalize(&hit));
      }
    }
    Err(import_not_found(url, &base, &searched))
  }

  fn is_current_file(&self, path: &Path) -> bool {
    self
      .current_file
      .iter()
      .any(|current| canonicalize(current) == path)
  }

  fn read_source(&self, path: &Path) -> Result<(String, PathBuf)> {
    let read = match &self.options.loader {
      Some(f) => f(path),
      None => std::fs::read_to_string(path),
    };
    match read {
      Ok(src) => Ok((src, path.to_path_buf())),
      Err(e) => Err(Error::io(format!(
        "Can't read `{}`: {}.",
        path.display(),
        e
      ))),
    }
  }

  /// `@use "url" as ns with (...)`
  fn do_use(&mut self, rule: &UseRule) -> Result<()> {
    let url = &rule.url;
    if let Some(builtin) = url
      .strip_prefix("vass:")
      .or_else(|| url.strip_prefix("sass:"))
    {
      let module = builtin_module(builtin);
      match &rule.namespace {
        Some(ns) if ns.is_empty() => self.env.merge_module_globals(&module),
        Some(ns) => {
          // remember the real module behind a custom alias so that
          // `ns.member` dispatches into the right built-in module
          self.builtin_ns.insert(ns.clone(), builtin.to_string());
          self.env.register_module(ns, module);
        }
        None => self.env.register_module(builtin, module),
      }
      return Ok(());
    }
    let module = self
      .load_module(url, rule)
      .map_err(|e| self.use_rule_context(e, rule))?;
    // The used module's CSS is emitted before this sheet's own output —
    // but only on the FIRST load (module singleton, reference behaviour)
    if self.just_loaded {
      self.module_css.extend(module.nodes.iter().cloned());
    }
    match &rule.namespace {
      Some(ns) if ns.is_empty() => self.env.merge_module_globals(&module),
      Some(ns) => self.env.register_module(ns, (*module).clone()),
      None => {
        let ns = default_namespace(url);
        self.env.register_module(&ns, (*module).clone());
      }
    }
    Ok(())
  }

  /// `@forward "url" as pre-* show/hide ...`
  fn do_forward(&mut self, rule: &UseRule) -> Result<()> {
    let url = &rule.url;
    if let Some(builtin) = url
      .strip_prefix("vass:")
      .or_else(|| url.strip_prefix("sass:"))
    {
      let module = builtin_module(builtin);
      self.forward_merge(Rc::new(module), rule, true);
      return Ok(());
    }
    let module = self
      .load_module(url, rule)
      .map_err(|e| self.use_rule_context(e, rule))?;
    let fresh = self.just_loaded;
    self.forward_merge(module, rule, fresh);
    Ok(())
  }

  /// Merge a forwarded module into the global scope with prefix/show/hide
  fn forward_merge(&mut self, module: Rc<Module>, rule: &UseRule, fresh: bool) {
    let prefix = rule.prefix.clone().unwrap_or_default();
    let visible = |name: &str| -> bool {
      if !rule.show.is_empty() && !rule.show.iter().any(|s| member_matches(s, name)) {
        return false;
      }
      if rule.hide.iter().any(|s| member_matches(s, name)) {
        return false;
      }
      true
    };
    let mut m = Module::default();
    for (k, v) in &module.variables {
      if visible(k) {
        m.variables.insert(format!("{}{}", prefix, k), v.clone());
      }
    }
    for (k, v) in &module.functions {
      if visible(k) {
        m.functions.insert(format!("{}{}", prefix, k), v.clone());
      }
    }
    for (k, v) in &module.mixins {
      if visible(k) {
        m.mixins.insert(format!("{}{}", prefix, k), v.clone());
      }
    }
    // Forwarded module CSS is emitted too — once per module
    if fresh {
      self.module_css.extend(module.nodes.iter().cloned());
    }
    // Forwarded members act as globals in the forwarding file
    self.env.merge_module_globals(&m);
    // Also register under the URL's default namespace for member access
    let ns = default_namespace(&rule.url);
    self.env.register_module(&ns, m);
  }

  /// Load a user module (with caching + cycle detection)
  fn load_module(&mut self, url: &str, rule: &UseRule) -> Result<Rc<Module>> {
    let path = self.resolve_import(url)?;
    if let Some(cached) = self.module_cache.get(&path) {
      // Configuring an already-loaded module is an error in the
      // reference (the configuration only applies on first load)
      if !rule.with.is_empty() {
        return Err(Error::import(
          "This module was already loaded, so it can't be configured using \"with\".",
        ));
      }
      self.just_loaded = false;
      return Ok(cached.clone());
    }
    self.just_loaded = true;
    if self.is_current_file(&path) {
      return Err(Error::import(format!(
        "Circular import cycle detected: {}.",
        path.display()
      )));
    }
    if self.loading.contains(&path) {
      return Err(Error::import(format!(
        "Module load cycle detected: {}.",
        path.display()
      )));
    }
    let (source, canonical) = self.read_source(&path)?;
    let syntax = vass_parser::detect_syntax(&canonical);
    let sheet = vass_parser::parse(&source, syntax)
      .map_err(|e| e.with_file(canonical.display().to_string()))?;

    self.loading.push(path.clone());
    self.current_file.push(canonical.clone());
    self.module_nesting += 1;

    // Fresh environment for the module; `with` config pre-sets variables
    let saved_env = std::mem::replace(&mut self.env, Environment::new());
    let saved_css = std::mem::take(&mut self.module_css);
    let saved_extends = std::mem::take(&mut self.extends);
    let saved_seen = std::mem::take(&mut self.seen_selectors);

    for (name, expr) in &rule.with {
      let v = self.eval_expr(expr)?;
      self.env.set(name, v, true);
    }

    let mut nodes = Vec::new();
    let compile_result = self.compile_stmts_root(&sheet.body, &Ctx::default(), &mut nodes);
    let seen = self.seen_selectors.clone();
    apply_extends(&mut nodes, &self.extends, &seen)?;

    let module_env = std::mem::replace(&mut self.env, saved_env);
    let module_css = std::mem::replace(&mut self.module_css, saved_css);
    self.extends = saved_extends;
    self.seen_selectors = saved_seen;
    self.current_file.pop();
    self.loading.pop();
    self.module_nesting -= 1;

    compile_result?;

    let mut module = Module {
      variables: module_env.global_variables(),
      functions: module_env.global_functions(),
      mixins: module_env.global_mixins(),
      nodes: module_css,
    };
    module.nodes.extend(nodes);
    let rc = Rc::new(module);
    self.module_cache.insert(canonical, rc.clone());
    Ok(rc)
  }
}

// ---- helpers ----------------------------------------------------------------

/// Canonicalize a path best-effort (no symlink resolution requirement)
fn canonicalize(p: &Path) -> PathBuf {
  match std::fs::canonicalize(p) {
    Ok(c) => c,
    Err(_) => p.to_path_buf(),
  }
}

/// Extensions tried when a module URL carries none (Vass adds `.vass`/
/// `.vcss` ahead of the reference's `.scss`/`.css`)
const MODULE_EXTS: [&str; 4] = ["vass", "vcss", "scss", "css"];

/// Probe one `<load-dir>/<url>` base path with dart-sass semantics.
///
/// Every probed path is appended to `searched` (for the failure help
/// text). `Ok(None)` means nothing matched here; `Err` is the dart-faithful
/// ambiguity error for a plain + partial pair with the same extension.
fn probe_candidates(base_path: &Path, searched: &mut Vec<String>) -> Result<Option<PathBuf>> {
  let dir = base_path
    .parent()
    .map(|d| d.to_path_buf())
    .unwrap_or_else(|| PathBuf::from("."));
  let name = base_path
    .file_name()
    .map(|n| n.to_string_lossy().to_string())
    .unwrap_or_default();
  if name.is_empty() {
    return Ok(None);
  }
  let probe = |p: PathBuf, searched: &mut Vec<String>| -> (bool, PathBuf) {
    searched.push(p.display().to_string());
    (p.is_file(), p)
  };

  // Explicit extension: the exact file plus its `_`-partial variant only
  // (dart-sass: `@use "base.scss"` also finds `_base.scss`)
  if let Some((stem, ext)) = split_known_ext(&name) {
    let plain = base_path.to_path_buf();
    let partial = dir.join(format!("_{}.{}", stem, ext));
    let (has_plain, plain) = probe(plain, searched);
    let (has_partial, partial) = probe(partial, searched);
    return match (has_plain, has_partial) {
      (false, false) => Ok(None),
      (true, false) => Ok(Some(plain)),
      (false, true) => Ok(Some(partial)),
      (true, true) => Err(ambiguous_import(&[plain, partial])),
    };
  }

  // Extensionless: per extension, plain then `_`-partial (a same-extension
  // pair is ambiguous), then the `index` directory form
  for ext in MODULE_EXTS {
    let plain = dir.join(format!("{}.{}", name, ext));
    let partial = dir.join(format!("_{}.{}", name, ext));
    let (has_plain, plain) = probe(plain, searched);
    let (has_partial, partial) = probe(partial, searched);
    if has_plain && has_partial {
      return Err(ambiguous_import(&[plain, partial]));
    }
    if has_plain {
      return Ok(Some(plain));
    }
    if has_partial {
      return Ok(Some(partial));
    }
  }
  for ext in MODULE_EXTS {
    let index = dir.join(&name).join(format!("index.{}", ext));
    let partial_index = dir.join(&name).join(format!("_index.{}", ext));
    let (has_index, index) = probe(index, searched);
    let (has_partial_index, partial_index) = probe(partial_index, searched);
    if has_index && has_partial_index {
      return Err(ambiguous_import(&[index, partial_index]));
    }
    if has_index {
      return Ok(Some(index));
    }
    if has_partial_index {
      return Ok(Some(partial_index));
    }
  }
  Ok(None)
}

/// Split `name` into `(stem, ext)` when it ends in a known module extension
fn split_known_ext(name: &str) -> Option<(&str, &str)> {
  for ext in MODULE_EXTS {
    let dot = format!(".{}", ext);
    if let Some(stem) = name.strip_suffix(&dot)
      && !stem.is_empty()
    {
      return Some((stem, ext));
    }
  }
  None
}

/// dart-faithful ambiguity error: two same-extension candidates for one URL
/// (`It's not clear which file to import. Found: _base.scss, base.scss`)
fn ambiguous_import(found: &[PathBuf]) -> Error {
  let list = found
    .iter()
    .map(|p| p.display().to_string())
    .collect::<Vec<_>>()
    .join(", ");
  Error::import("It's not clear which file to import.").with_help(format!("Found: {}", list))
}

/// E0008 with a compact summary of every searched location, plus a hint
/// when a same-named candidate exists outside the search directories (the
/// classic "file saved in the wrong folder" mistake).
fn import_not_found(url: &str, base: &Path, searched: &[String]) -> Error {
  let name = Path::new(url)
    .file_name()
    .map(|n| n.to_string_lossy().to_string())
    .unwrap_or_else(|| url.to_string());
  let (stem, explicit_ext) = split_known_ext(&name)
    .map(|(s, e)| (s, Some(e)))
    .unwrap_or((name.as_str(), None));

  let patterns: Vec<String> = match explicit_ext {
    Some(ext) => vec![format!("{}.{}", stem, ext), format!("_{stem}.{ext}")],
    None => {
      let mut p = Vec::new();
      for ext in MODULE_EXTS {
        p.push(format!("{}.{}", stem, ext));
      }
      for ext in MODULE_EXTS {
        p.push(format!("_{stem}.{ext}"));
      }
      for ext in MODULE_EXTS {
        p.push(format!("{stem}/index.{ext}"));
      }
      for ext in MODULE_EXTS {
        p.push(format!("{stem}/_index.{ext}"));
      }
      p
    }
  };

  // Unique directories that were actually probed
  let mut dirs: Vec<String> = Vec::new();
  for p in searched {
    if let Some(d) = Path::new(p).parent() {
      let d = d.display().to_string();
      if !dirs.contains(&d) {
        dirs.push(d);
      }
    }
  }
  let dirs_shown = if dirs.is_empty() {
    "(no load paths)".to_string()
  } else {
    dirs.join(", ")
  };

  // Nearby hint: same-named file one level up or in the working directory
  let mut hint_dirs: Vec<PathBuf> = Vec::new();
  if let Some(parent) = base.parent() {
    hint_dirs.push(parent.to_path_buf());
  }
  if let Ok(cwd) = std::env::current_dir() {
    hint_dirs.push(cwd);
  }
  let mut nearby: Option<PathBuf> = None;
  'outer: for dir in hint_dirs {
    if dir == base || dir.as_os_str().is_empty() {
      continue;
    }
    let candidates: Vec<PathBuf> = match explicit_ext {
      Some(ext) => vec![
        dir.join(format!("{}.{}", stem, ext)),
        dir.join(format!("_{stem}.{ext}")),
      ],
      None => MODULE_EXTS
        .iter()
        .flat_map(|e| {
          vec![
            dir.join(format!("{}.{}", stem, e)),
            dir.join(format!("_{stem}.{e}")),
          ]
        })
        .collect(),
    };
    if let Some(hit) = candidates.into_iter().find(|c| c.is_file()) {
      nearby = Some(hit);
      break 'outer;
    }
  }

  // The suggested file to create: partial convention + Vass's native ext
  let suggested = match explicit_ext {
    Some(ext) => format!("_{stem}.{ext}"),
    None => format!("_{stem}.vcss"),
  };
  let mut err = Error::import(format!("Can't find module `{}`.", url));
  err = match &nearby {
    Some(hit) => {
      // Also surfaced as LSP relatedInformation (note carries the file)
      err
        .with_help(format!(
          "found `{hit}` nearby - @use resolves relative to the importing \
                 file's directory (`{base}`), not the working directory",
          hit = hit.display(),
          base = base.display(),
        ))
        .with_note_in_file(
          "a module with this name exists here - move it next to the importing \
                 file, use a relative URL like `@use '../base';`, or add its folder \
                 with `--load-path`",
          hit.display().to_string(),
        )
    }
    None => err.with_help(format!(
      "create the module file `{suggested}` next to the importing file (in \
             `{base}`), or fix the URL if the module lives elsewhere",
      base = base.display()
    )),
  };
  // The full probe list stays a NOTE so the actionable help line remains
  // the first thing a user reads.
  err.with_note(format!(
    "searched for {} in {}",
    patterns.join(", "),
    dirs_shown
  ))
}

/// Default namespace for a module URL: basename minus `_` and extension,
/// minus any leading `vass:` prefix
fn default_namespace(url: &str) -> String {
  if let Some(rest) = url.strip_prefix("vass:") {
    return rest.to_string();
  }
  let base = url.rsplit('/').next().unwrap_or(url);
  let base = base.strip_prefix('_').unwrap_or(base);
  match base.rfind('.') {
    Some(i) if i > 0 => base[..i].to_string(),
    _ => base.to_string(),
  }
}

/// Whether a `@forward` show/hide member pattern matches a member name
fn member_matches(pattern: &str, name: &str) -> bool {
  let pattern = pattern.trim_start_matches('$');
  if let Some(prefix) = pattern.strip_suffix("-*") {
    return name.starts_with(prefix);
  }
  if pattern == "*" {
    return true;
  }
  pattern == name
}

/// Build the runtime module for a `vass:` builtin
fn builtin_module(name: &str) -> Module {
  let mut m = Module::default();
  for (k, v) in builtins::module_variables(name) {
    m.variables.insert(k.to_string(), v);
  }
  m
}

/// Whether a name is a known global builtin (for function-exists)
fn builtins_known_function(name: &str) -> bool {
  [
    "rgb",
    "rgba",
    "hsl",
    "hsla",
    "lighten",
    "darken",
    "saturate",
    "desaturate",
    "grayscale",
    "adjust-hue",
    "complement",
    "invert",
    "mix",
    "red",
    "green",
    "blue",
    "hue",
    "saturation",
    "lightness",
    "alpha",
    "opacity",
    "opacify",
    "fade-in",
    "transparentize",
    "fade-out",
    "adjust-color",
    "scale-color",
    "change-color",
    "ie-hex-str",
    "percentage",
    "round",
    "ceil",
    "floor",
    "abs",
    "min",
    "max",
    "random",
    "comparable",
    "unit",
    "unitless",
    "unquote",
    "quote",
    "to-upper-case",
    "to-lower-case",
    "str-length",
    "str-insert",
    "str-index",
    "str-slice",
    "unique-id",
    "length",
    "nth",
    "set-nth",
    "join",
    "append",
    "zip",
    "index",
    "list-separator",
    "is-bracketed",
    "map-get",
    "map-merge",
    "map-remove",
    "map-keys",
    "map-values",
    "map-has-key",
    "type-of",
    "inspect",
    "keywords",
    "if",
    "selector-nest",
    "selector-append",
    "selector-unify",
    "selector-parse",
    "selector-replace",
    "selector-superselector",
    "variable-exists",
    "global-variable-exists",
    "function-exists",
    "mixin-exists",
    "content-exists",
    "feature-exists",
    "get-function",
    "call",
  ]
  .contains(&name)
}

/// Legacy component-wise color arithmetic (`+` / `-`). Channels are computed
/// on the 0-255 scale, alpha on 0-1; results clamp into range and lose the
/// literal representation (they are computed colors).
fn color_component_op(
  a: &vass_core::value::Color,
  b: &vass_core::value::Color,
  sub: bool,
) -> vass_core::value::Color {
  let clamp255 = |v: f64| v.clamp(0.0, 255.0);
  let (r1, g1, b1) = (a.red(), a.green(), a.blue());
  let (r2, g2, b2) = (b.red(), b.green(), b.blue());
  let (r, g, bl) = if sub {
    (clamp255(r1 - r2), clamp255(g1 - g2), clamp255(b1 - b2))
  } else {
    (clamp255(r1 + r2), clamp255(g1 + g2), clamp255(b1 + b2))
  };
  let alpha = if sub {
    (a.alpha() - b.alpha()).clamp(0.0, 1.0)
  } else {
    (a.alpha() + b.alpha()).clamp(0.0, 1.0)
  };
  vass_core::value::Color::from_channels(r, g, bl, alpha)
}

/// Multiply (or divide by `1/n`) every channel of a color by a scalar.
fn scale_color_channels(a: &vass_core::value::Color, n: f64) -> vass_core::value::Color {
  let clamp255 = |v: f64| v.clamp(0.0, 255.0);
  vass_core::value::Color::from_channels(
    clamp255(a.red() * n),
    clamp255(a.green() * n),
    clamp255(a.blue() * n),
    a.alpha().clamp(0.0, 1.0),
  )
}

/// Whether the right side of a `+` may be a string (for concatenation)
fn expr_may_be_string(e: &Expr) -> bool {
  matches!(
    e,
    Expr::Str { .. } | Expr::Variable { .. } | Expr::Interp { .. } | Expr::InterpValue { .. }
  )
}

/// Simplify `calc(...)` / `clamp(...)` written as plain-CSS function text
/// when every operand is a resolvable number (`calc(1px + 2px)` -> `3px`,
/// `clamp(1px, 5px, 3px)` -> `3px`). Anything the static evaluator cannot
/// resolve (var(), incompatible units, nested specials) stays verbatim,
/// matching the reference compiler's calculation simplification.
fn simplify_calc_like(value: &Value) -> Result<Value> {
  let text = match value {
    Value::Str(s) if !s.quoted => &s.text,
    _ => return Ok(value.clone()),
  };
  let trimmed = text.trim_start();
  let (name, inner) = if let Some(rest) = trimmed
    .strip_prefix("calc(")
    .and_then(|r| r.strip_suffix(')'))
  {
    ("calc", rest)
  } else if let Some(rest) = trimmed
    .strip_prefix("clamp(")
    .and_then(|r| r.strip_suffix(')'))
  {
    ("clamp", rest)
  } else {
    return Ok(value.clone());
  };
  // Values that embed special CSS functions can never simplify.
  if inner.contains("var(")
    || inner.contains("calc(")
    || inner.contains("env(")
    || inner.contains("clamp(")
    || inner.contains("min(")
    || inner.contains("max(")
    || inner.contains("calc(")
  {
    return Ok(value.clone());
  }
  let result = match name {
    "calc" => simplify_calc_expr(inner),
    _ => simplify_clamp_args(inner),
  };
  Ok(match result {
    Some(v) => v,
    None => value.clone(),
  })
}

/// Evaluate a calc() inner expression: split on top-level +/-, evaluate each
/// term's * / chain with Number arithmetic; any failure keeps the raw text.
fn simplify_calc_expr(inner: &str) -> Option<Value> {
  let terms = split_top_level(inner, &['+', '-'])?;
  if terms.is_empty() {
    return None;
  }
  let mut total = eval_calc_term(&terms[0].text, terms[0].sign)?;
  for t in terms.iter().skip(1) {
    let rhs = eval_calc_term(&t.text, 1.0)?;
    total = match t.sign {
      1.0 => total.add(&rhs).ok()?,
      _ => total.sub(&rhs).ok()?,
    };
  }
  Some(Value::Number(total))
}

struct CalcTerm {
  sign: f64,
  text: String,
}

/// Split `a + b - c` (or a `*` / `/` chain) into signed terms at paren/bracket
/// depth 0. `+`/`-` require surrounding whitespace (CSS calc rule and unit
/// signs); `*`/`/` split regardless of whitespace. Sign encodes the operator:
/// 1.0 keeps/`*`, -1.0 subtracts/`/`.
fn split_top_level(text: &str, ops: &[char]) -> Option<Vec<CalcTerm>> {
  let is_additive = ops.contains(&'+');
  let chars: Vec<char> = text.chars().collect();
  let mut terms: Vec<CalcTerm> = Vec::new();
  let mut depth = 0i32;
  let mut start = 0usize;
  let mut pending_sign = 1.0f64;
  let mut i = 0usize;
  while i < chars.len() {
    match chars[i] {
      '(' | '[' => depth += 1,
      ')' | ']' => depth -= 1,
      c if depth == 0 && ops.contains(&c) => {
        let split_here = if is_additive {
          // `+`/`-` operators need whitespace on both sides
          let prev_ws = i > 0 && chars[i - 1].is_whitespace();
          let next_ws = chars
            .get(i + 1)
            .map(|ch| ch.is_whitespace())
            .unwrap_or(false);
          prev_ws && next_ws
        } else {
          true
        };
        if split_here {
          let term = chars[start..i]
            .iter()
            .collect::<String>()
            .trim()
            .to_string();
          if !term.is_empty() {
            terms.push(CalcTerm {
              sign: pending_sign,
              text: term,
            });
          }
          pending_sign = if c == '-' || c == '/' { -1.0 } else { 1.0 };
          start = i + 1;
        }
      }
      _ => {}
    }
    i += 1;
  }
  let term = chars[start..].iter().collect::<String>().trim().to_string();
  if term.is_empty() {
    return if terms.is_empty() { None } else { Some(terms) };
  }
  terms.push(CalcTerm {
    sign: pending_sign,
    text: term,
  });
  Some(terms)
}

/// Evaluate one term: a product/quotient chain of numeric factors
fn eval_calc_term(text: &str, sign: f64) -> Option<Number> {
  let factors = split_top_level(text, &['*', '/'])?;
  if factors.is_empty() {
    return None;
  }
  let first = eval_calc_factor(&factors[0].text, factors[0].sign)?;
  let mut acc = first;
  for f in factors.iter().skip(1) {
    let rhs = eval_calc_factor(&f.text, 1.0)?;
    // The split preserved the operator as the term's sign
    acc = match f.sign {
      -1.0 => acc.div(&rhs).ok()?,
      _ => acc.mul(&rhs).ok()?,
    };
  }
  let _ = sign;
  Some(acc)
}

/// Parse one factor as a bare number (`12px`, `50%`, `-3`)
fn eval_calc_factor(text: &str, sign: f64) -> Option<Number> {
  let t = text.trim();
  let sheet = vass_parser::parse_expr_snippet(t).ok()?;
  let expr = sheet;
  match expr {
    Expr::Number { value, unit } => Some(Number::new(sign * value, unit)),
    _ => None,
  }
}

/// clamp(min, value, max) with plain numbers picks one of the bounds
fn simplify_clamp_args(inner: &str) -> Option<Value> {
  let parts: Vec<&str> = inner.split(',').collect();
  if parts.len() != 3 {
    return None;
  }
  let min = eval_calc_factor(parts[0], 1.0)?;
  let val = eval_calc_factor(parts[1], 1.0)?;
  let max = eval_calc_factor(parts[2], 1.0)?;
  // Only simplify when all three operands are comparable: `clamp(1px,
  // 2vw, 3px)` has no static answer and must stay a calc() call
  if !min.compatible(&val) || !min.compatible(&max) || !val.compatible(&max) {
    return None;
  }
  let chosen = if val < min {
    min
  } else if val > max {
    max
  } else {
    val
  };
  Some(Value::Number(chosen))
}

/// Parse a modern CSS color function call (`oklch()`, `oklab()`, `lab()`,
/// `lch()`, `hwb()`, `color()`) from evaluated argument values. Returns
/// `None` when the arguments are not statically resolvable (var() etc.), in
/// which case the call stays a plain-CSS passthrough.
fn try_modern_color(name: &str, evaluated: &[Value]) -> Option<vass_core::value::Color> {
  use vass_core::value::{ColorSpace, ModernColor};

  // Flatten space-separated list arguments (channels arrive as a space list)
  let mut vals: Vec<Value> = Vec::new();
  for v in evaluated {
    if let Value::List(l) = v
      && l.sep == Separator::Space
    {
      vals.extend(l.items.iter().cloned());
      continue;
    }
    vals.push(v.clone());
  }

  /// Parse a channel fragment of a `num/alpha` slash pair back into a
  /// value (`50%` -> Number(50, "%"), `none` -> the unquoted string)
  fn channel_text_value(text: &str) -> Option<Value> {
    let t = text.trim();
    if t.eq_ignore_ascii_case("none") {
      return Some(Value::Str(VassString::unquoted("none")));
    }
    if let Some(p) = t.strip_suffix('%') {
      return p
        .parse::<f64>()
        .ok()
        .map(|v| Value::Number(Number::new(v, Some("%".to_string()))));
    }
    t.parse::<f64>()
      .ok()
      .map(|v| Value::Number(Number::unitless(v)))
  }

  /// Parse an alpha fragment after `/`: `none` marks a missing alpha,
  /// percentages scale into 0-1, plain numbers stay as-is
  fn alpha_text_value(text: &str) -> Option<Option<f64>> {
    let t = text.trim();
    if t.eq_ignore_ascii_case("none") {
      return Some(None);
    }
    if let Some(p) = t.strip_suffix('%') {
      return p.parse::<f64>().ok().map(|v| Some(v / 100.0));
    }
    t.parse::<f64>().ok().map(Some)
  }

  fn channel_value(v: &Value, space: ColorSpace, idx: usize) -> Option<Option<f64>> {
    let cname = space.channels()[idx];
    match v {
      Value::Number(n) => {
        // Percentages scale into 0-1 storage; hue accepts any angle
        // unit (unitless hue is degrees); other unitless channels are
        // stored raw (0-1 scales like L/C, unbounded a/b).
        let val = match (cname, n.unit.as_deref()) {
          ("hue", Some("grad")) => n.value * 360.0 / 400.0,
          ("hue", Some("rad")) => n.value.to_degrees(),
          ("hue", Some("turn")) => n.value * 360.0,
          ("hue", Some("deg")) | ("hue", None) => n.value,
          (_, Some("%")) => n.value / 100.0,
          (_, None) => n.value,
          _ => return None,
        };
        Some(Some(val))
      }
      Value::Str(s) if !s.quoted && s.text == "none" => Some(None),
      // slash-joined `0.5/0.8` alpha fragments are handled separately
      _ => None,
    }
  }

  let space = match name.to_ascii_lowercase().as_str() {
    "oklch" => ColorSpace::Oklch,
    "oklab" => ColorSpace::Oklab,
    "lab" => ColorSpace::Lab,
    "lch" => ColorSpace::Lch,
    "hwb" => ColorSpace::Hwb,
    "color" => {
      // color(SPACE r g b [/ alpha])
      let sname = match vals.first() {
        Some(Value::Str(s)) if !s.quoted => s.text.clone(),
        _ => return None,
      };
      let space = ColorSpace::from_name(&sname)?;
      let mut ch: [Option<f64>; 3] = [None, None, None];
      let mut alpha: Option<f64> = Some(1.0);
      let mut ci = 0usize;
      for v in vals.iter().skip(1) {
        if let Value::Str(s) = v
          && !s.quoted
          && let Some((num, a)) = s.text.split_once('/')
        {
          let fake = channel_text_value(num)?;
          if ci < 3 {
            ch[ci] = channel_value(&fake, space, ci)?;
            ci += 1;
          }
          alpha = alpha_text_value(a)?;
          continue;
        }
        if matches!(v, Value::Number(_) | Value::Str(_)) {
          if ci < 3 {
            ch[ci] = channel_value(v, space, ci)?;
            ci += 1;
          }
          // an extra 4th positional value is the alpha
          else if let Value::Number(n) = v {
            alpha = Some(n.value);
          }
        }
      }
      let mc = ModernColor::new(space, ch, alpha);
      let mc = normalize_hwb_channels(mc);
      return Some(mc.into_color());
    }
    _ => return None,
  };

  // oklch/oklab/lab/lch/hwb: three channels + optional slash alpha in the
  // last space-list item (`oklch(50% 0.1 200 / 0.5)`)
  let mut ch: [Option<f64>; 3] = [None, None, None];
  let mut alpha: Option<f64> = Some(1.0);
  let mut ci = 0usize;
  for v in &vals {
    // slash alpha arrives glued to the last channel as `200/0.5`
    if let Value::Str(s) = v
      && !s.quoted
      && let Some((num, a)) = s.text.split_once('/')
    {
      let fake = channel_text_value(num)?;
      if ci < 3 {
        ch[ci] = channel_value(&fake, space, ci)?;
        ci += 1;
      }
      alpha = alpha_text_value(a)?;
      continue;
    }
    if ci < 3 {
      ch[ci] = channel_value(v, space, ci)?;
      ci += 1;
    } else if let Value::Number(n) = v {
      alpha = Some(n.value);
    }
  }
  if ci < 3 {
    return None;
  }
  let mc = ModernColor::new(space, ch, alpha);
  let mc = normalize_hwb_channels(mc);
  Some(mc.into_color())
}

/// The reference normalizes hwb whiteness/blackness at construction when
/// their sum exceeds 1 (`hwb(0 150% 150%)` is stored as `hwb(0 50% 50%)`)
fn normalize_hwb_channels(mut m: vass_core::value::ModernColor) -> vass_core::value::ModernColor {
  if m.space != ColorSpace::Hwb {
    return m;
  }
  let (w, b) = (m.ch[1].unwrap_or(0.0), m.ch[2].unwrap_or(0.0));
  if w + b > 1.0 {
    let sum = w + b;
    m.ch[1] = m.ch[1].map(|v| v / sum);
    m.ch[2] = m.ch[2].map(|v| v / sum);
  }
  m
}

/// Whether a value is (or contains, inside a space list) an unquoted string
/// holding a special CSS function — `var()`, `env()`, `calc()` or `clamp()`.
/// Such arguments make static color resolution impossible, so calls like
/// `rgb(var(--accent), 0.5)` must be emitted verbatim.
fn contains_special_css_fn(v: &Value) -> bool {
  match v {
    Value::Str(s) => {
      !s.quoted
        && (s.text.contains("var(")
          || s.text.contains("env(")
          || s.text.contains("calc(")
          || s.text.contains("clamp("))
    }
    Value::List(l) => l.items.iter().any(contains_special_css_fn),
    _ => false,
  }
}

fn value_to_concat_text(v: &Value) -> String {
  match v {
    // String contents join without their quotes; the result's quoting
    // is decided separately by the left operand
    Value::Str(s) => s.text.clone(),
    // null contributes nothing in string contexts (reference: "a" + null
    // is "a" and a-#{null}-b is a--b)
    Value::Null => String::new(),
    other => other.to_css(),
  }
}

/// Split `text` on top-level `sep` characters (outside parens, brackets
/// and quoted strings) — used for media query lists
fn split_media_top_level(text: &str, sep: char) -> Vec<String> {
  let mut parts = Vec::new();
  let mut depth = 0i32;
  let mut quote: Option<char> = None;
  let mut esc = false;
  let mut cur = String::new();
  for ch in text.chars() {
    if esc {
      esc = false;
      cur.push(ch);
      continue;
    }
    match quote {
      Some(q) => {
        if ch == '\\' {
          esc = true;
        } else if ch == q {
          quote = None;
        }
        cur.push(ch);
      }
      None => match ch {
        '"' | '\'' => {
          quote = Some(ch);
          cur.push(ch);
        }
        '(' | '[' => {
          depth += 1;
          cur.push(ch);
        }
        ')' | ']' => {
          depth -= 1;
          cur.push(ch);
        }
        c if c == sep && depth <= 0 => parts.push(std::mem::take(&mut cur)),
        _ => cur.push(ch),
      },
    }
  }
  parts.push(cur);
  parts
}

/// Find the first `c` at nesting depth 0 (outside parens/brackets/strings)
fn find_top_level_char(text: &str, c: char) -> Option<usize> {
  let mut depth = 0i32;
  let mut quote: Option<char> = None;
  let mut esc = false;
  for (i, ch) in text.char_indices() {
    if esc {
      esc = false;
      continue;
    }
    match quote {
      Some(q) => {
        if ch == '\\' {
          esc = true;
        } else if ch == q {
          quote = None;
        }
      }
      None => match ch {
        '"' | '\'' => quote = Some(ch),
        '(' | '[' => depth += 1,
        ')' | ']' => depth -= 1,
        x if x == c && depth == 0 => return Some(i),
        _ => {}
      },
    }
  }
  None
}

/// Match a leading `(...)` in `part`; returns (inner, text-after-the-paren)
fn match_media_parens(part: &str) -> Option<(&str, &str)> {
  let mut depth = 0i32;
  let mut quote: Option<char> = None;
  let mut esc = false;
  for (i, ch) in part.char_indices() {
    if esc {
      esc = false;
      continue;
    }
    match quote {
      Some(q) => {
        if ch == '\\' {
          esc = true;
        } else if ch == q {
          quote = None;
        }
      }
      None => match ch {
        '"' | '\'' => quote = Some(ch),
        '(' => depth += 1,
        ')' => {
          depth -= 1;
          if depth == 0 {
            return Some((&part[1..i], &part[i + 1..]));
          }
        }
        _ => {}
      },
    }
  }
  None
}

/// Split a media query on top-level `and`/`or` keywords (outside parens and
/// quotes). Returns `(preceding keyword lowercased, part)` pairs.
fn split_media_on_keywords(seg: &str) -> Vec<(Option<&'static str>, String)> {
  let mut parts: Vec<(Option<&'static str>, String)> = Vec::new();
  let mut pending_kw: Option<&'static str> = None;
  let mut seg_start = 0usize;
  let mut i = 0usize;
  let mut depth = 0i32;
  let mut quote: Option<char> = None;
  let mut esc = false;
  let b = seg.as_bytes();
  while i < b.len() {
    let ch = seg[i..].chars().next().unwrap();
    if esc {
      esc = false;
      i += ch.len_utf8();
      continue;
    }
    match quote {
      Some(q) => {
        if ch == '\\' {
          esc = true;
        } else if ch == q {
          quote = None;
        }
        i += ch.len_utf8();
      }
      None => match ch {
        '"' | '\'' => {
          quote = Some(ch);
          i += 1;
        }
        '(' | '[' => {
          depth += 1;
          i += 1;
        }
        ')' | ']' => {
          depth -= 1;
          i += 1;
        }
        _ if depth == 0 && matches!(ch, 'a' | 'A' | 'o' | 'O') => {
          let rest = &b[i..];
          let (kw, kwlen) = if rest.len() >= 3 && rest[..3].eq_ignore_ascii_case(b"and") {
            (Some("and"), 3)
          } else if rest.len() >= 2 && rest[..2].eq_ignore_ascii_case(b"or") {
            (Some("or"), 2)
          } else {
            (None, 0)
          };
          let prev_ok = i == 0 || seg[..i].chars().last().is_some_and(|c| c.is_whitespace());
          let after = i + kwlen;
          let next_ok = after >= seg.len()
            || seg[after..]
              .chars()
              .next()
              .is_some_and(|c| c.is_whitespace());
          if let (Some(k), true, true) = (kw, prev_ok, next_ok) {
            parts.push((pending_kw, seg[seg_start..i].trim().to_string()));
            pending_kw = Some(k);
            seg_start = after;
            i = after;
          } else {
            i += ch.len_utf8();
          }
        }
        _ => {
          i += ch.len_utf8();
        }
      },
    }
  }
  parts.push((pending_kw, seg[seg_start..].trim().to_string()));
  parts
}

/// Combine parent selectors with a child selector (cartesian product over
/// commas; `&` replaced by the parent; otherwise descendant combinator)
fn resolve_child_selectors(parents: &[String], child: &str) -> Result<Vec<String>> {
  validate_parent_positions(child)?;
  let child_parts: Vec<&str> = child.split(',').map(|s| s.trim()).collect();
  if parents.is_empty() {
    // A parent selector with no parent in scope is an error
    // (reference: `Top-level selectors may not contain the parent
    // selector "&".`) — only when the selector actually starts with one
    if child.trim_start().starts_with('&') {
      return Err(Error::syntax(
        "Top-level selectors may not contain the parent selector \"&\".",
      ));
    }
    return Ok(
      child_parts
        .into_iter()
        .map(normalize_attr_selector_values)
        .collect(),
    );
  }
  let mut result = Vec::new();
  for part in &child_parts {
    let amp_count = part.matches('&').count();
    if amp_count == 0 {
      for parent in parents {
        result.push(normalize_attr_selector_values(&format!(
          "{} {}",
          parent, part
        )));
      }
    } else if amp_count == 1 {
      for parent in parents {
        result.push(normalize_attr_selector_values(&part.replace('&', parent)));
      }
    } else {
      // Multiple `&` (in DIFFERENT compounds, e.g. `.a, .b { & + & }`):
      // each position independently ranges over the parent list — the
      // cartesian product (`.a, .b { & + & }` →
      // `.a + .a, .a + .b, .b + .a, .b + .b`)
      for combo in cartesian(parents, amp_count) {
        let mut s = part.to_string();
        for p in &combo {
          s = s.replacen('&', p, 1);
        }
        result.push(normalize_attr_selector_values(&s));
      }
    }
  }
  Ok(result)
}

/// The reference rejects a parent selector that is not at the beginning of
/// its compound selector: `&&`, `.a&`, `&.a&` all error with
/// `"&" may only used at the beginning of a compound selector.` (sic).
/// Combinators (whitespace, `>`, `+`, `~`, `,`) start a new compound;
/// brackets/parens protect their contents.
fn validate_parent_positions(child: &str) -> Result<()> {
  let mut depth = 0i32;
  let mut compound_has_amp = false;
  let mut compound_started = false;
  for c in child.chars() {
    match c {
      '[' | '(' => {
        depth += 1;
        compound_started = true;
      }
      ']' | ')' => {
        depth -= 1;
      }
      '&' if depth == 0 => {
        if compound_started || compound_has_amp {
          return Err(Error::syntax(
            "\"&\" may only used at the beginning of a compound selector.",
          ));
        }
        compound_has_amp = true;
        compound_started = true;
      }
      '>' | '+' | '~' | ',' if depth == 0 => {
        compound_started = false;
        compound_has_amp = false;
      }
      c if c.is_whitespace() && depth == 0 => {
        // whitespace is a descendant-combinator only when the next
        // significant char is not another combinator; treating it as
        // a boundary is safe for `&`-position validation
        compound_started = false;
        compound_has_amp = false;
      }
      _ => {
        compound_started = true;
      }
    }
  }
  Ok(())
}

/// Normalize attribute-selector VALUES the way the reference parses and
/// re-serializes selectors: a quoted value that spells a valid CSS ident
/// is written unquoted (`[x='y']` -> `[x=y]`); otherwise it is re-quoted
/// preferring double quotes (`[x='a"b']` -> `[x='a"b']`, `[x='a b']` ->
/// `[x="a b"]`). Bare values pass through untouched.
fn normalize_attr_selector_values(sel: &str) -> String {
  let bytes = sel.as_bytes();
  let mut out = String::with_capacity(sel.len());
  let mut i = 0;
  while i < bytes.len() {
    match bytes[i] {
      b'"' | b'\'' => {
        // skip over a quoted string in the selector verbatim
        let quote = bytes[i];
        out.push(sel[i..].chars().next().unwrap());
        i += 1;
        while i < bytes.len() {
          if bytes[i] == b'\\' && i + 1 < bytes.len() {
            out.push(bytes[i] as char);
            out.push(bytes[i + 1] as char);
            i += 2;
            continue;
          }
          let c = sel[i..].chars().next().unwrap();
          out.push(c);
          i += c.len_utf8();
          if c as u32 == quote as u32 {
            break;
          }
        }
      }
      b'[' => {
        // find the matching `]` (quote-aware) and rewrite its value
        let mut j = i + 1;
        let mut inner = String::new();
        let mut closed = false;
        while j < bytes.len() {
          let c = sel[j..].chars().next().unwrap();
          if c == '\\' && j + 1 < bytes.len() {
            inner.push(c);
            let n = sel[j + 1..].chars().next().unwrap();
            inner.push(n);
            j += 1 + n.len_utf8();
            continue;
          }
          if c == '"' || c == '\'' {
            let q = c;
            inner.push(c);
            j += 1;
            while j < bytes.len() {
              let d = sel[j..].chars().next().unwrap();
              if d == '\\' && j + 1 < bytes.len() {
                inner.push(d);
                let n = sel[j + 1..].chars().next().unwrap();
                inner.push(n);
                j += 1 + n.len_utf8();
                continue;
              }
              inner.push(d);
              j += d.len_utf8();
              if d == q {
                break;
              }
            }
            continue;
          }
          if c == ']' {
            j += 1;
            closed = true;
            break;
          }
          inner.push(c);
          j += c.len_utf8();
        }
        if closed {
          out.push('[');
          out.push_str(&rewrite_attr_value(&inner));
          out.push(']');
          i = j;
        } else {
          // unterminated: emit verbatim
          out.push('[');
          out.push_str(&inner);
          i = j;
        }
      }
      _ => {
        let c = sel[i..].chars().next().unwrap();
        out.push(c);
        i += c.len_utf8();
      }
    }
  }
  out
}

/// Rewrite one `[...]` inner text so its value part follows the reference
/// serialization rules. Inner shape: `ident [op value] [flags]`.
fn rewrite_attr_value(inner: &str) -> String {
  let trimmed = inner.trim();
  // find the operator (attribute mutators), skipping quoted spans
  let bytes = trimmed.as_bytes();
  let mut op_pos = None;
  let mut k = 0;
  while k < bytes.len() {
    let c = bytes[k];
    if c == b'"' || c == b'\'' {
      let q = c;
      k += 1;
      while k < bytes.len() {
        if bytes[k] == b'\\' {
          k += 2;
          continue;
        }
        if bytes[k] == q {
          k += 1;
          break;
        }
        k += 1;
      }
      continue;
    }
    if matches!(c, b'~' | b'|' | b'^' | b'$' | b'*') && k + 1 < bytes.len() && bytes[k + 1] == b'='
    {
      op_pos = Some((k, k + 2));
      break;
    }
    if c == b'=' {
      op_pos = Some((k, k + 1));
      break;
    }
    k += 1;
  }
  let Some((op_start, val_start)) = op_pos else {
    // no operator (`[disabled]`): normalize surrounding whitespace only
    return trimmed.to_string();
  };
  let op = &trimmed[op_start..val_start];
  let rest = trimmed[val_start..].trim_start();
  // split trailing flags (i/s) off the value
  let (value_part, flags) = split_attr_flags(rest);
  let rewritten = match unquote_attr_value(&value_part) {
    Some(decoded) => {
      if is_css_ident(&decoded) {
        decoded
      } else {
        quote_attr_value(&decoded)
      }
    }
    None => value_part.trim().to_string(),
  };
  let attr = trimmed[..op_start].trim();
  let mut s = String::with_capacity(inner.len() + 4);
  s.push_str(attr);
  s.push_str(op);
  s.push_str(&rewritten);
  if let Some(f) = flags {
    s.push(' ');
    s.push_str(f);
  }
  s
}

/// Split a trailing case flag (`i`/`s`) from an attribute value; returns
/// `(value, Option<flag>)`
fn split_attr_flags(rest: &str) -> (String, Option<&str>) {
  let t = rest.trim();
  if t.len() >= 2 {
    let last = t.chars().last().unwrap();
    if last == 'i' || last == 's' || last == 'I' || last == 'S' {
      let head = t[..t.len() - 1].trim_end();
      if !head.is_empty() {
        return (head.to_string(), Some(&t[t.len() - 1..]));
      }
    }
  }
  (t.to_string(), None)
}

/// Decode a quoted attribute value; `None` when the value is not quoted
fn unquote_attr_value(v: &str) -> Option<String> {
  let t = v.trim();
  let b = t.as_bytes();
  if b.len() < 2 || (b[0] != b'"' && b[0] != b'\'') {
    return None;
  }
  let quote = b[0];
  if b[b.len() - 1] != quote {
    return None;
  }
  let body = &t[1..t.len() - 1];
  let mut out = String::with_capacity(body.len());
  let mut chars = body.chars().peekable();
  let quote_char = quote as char;
  while let Some(c) = chars.next() {
    if c == '\\' {
      match chars.peek() {
        Some(n) if *n == quote_char || *n == '\\' => {
          out.push(*n);
          chars.next();
        }
        _ => out.push(c),
      }
    } else {
      out.push(c);
    }
  }
  Some(out)
}

/// Re-quote an attribute value, preferring double quotes; values containing
/// `"` use single quotes (`a"b` -> `'a"b'`)
fn quote_attr_value(decoded: &str) -> String {
  if decoded.contains('"') {
    format!("'{}'", decoded.replace('\\', "\\\\").replace('\'', "\\'"))
  } else {
    format!("\"{}\"", decoded.replace('\\', "\\\\").replace('"', "\\\""))
  }
}

/// Whether `s` is a valid CSS identifier per the CSS Syntax Level 3
/// <ident-token> grammar as the reference applies it to attribute values:
/// letters/`_`/non-ASCII start, `-` requires a name-start after it
/// (`-a` yes, `-1` no), and `--`-style custom idents stay QUOTED in the
/// reference's attribute output (`[b="--a"]` keeps quotes).
fn is_css_ident(s: &str) -> bool {
  if s.is_empty() {
    return false;
  }
  let mut chars = s.chars();
  let first = chars.next().unwrap();
  if first == '-' {
    let second = match chars.next() {
      Some(c) => c,
      None => return false, // a lone `-` is not an ident
    };
    // `--` custom-ident style values stay quoted in the reference
    if second == '-' {
      return false;
    }
    // `-a` is an ident; `-1` is not (a digit is not a name-start)
    return is_name_start(second);
  }
  is_name_start(first) && s.chars().skip(1).all(is_name_char)
}

fn is_name_start(c: char) -> bool {
  c.is_alphabetic() || c == '_' || (c as u32) >= 0x80
}

fn is_name_char(c: char) -> bool {
  is_name_start(c) || c.is_ascii_digit() || c == '-'
}

/// All ordered combinations of `k` parents (cartesian product with
/// repetition), outer loop = leftmost position
fn cartesian(parents: &[String], k: usize) -> Vec<Vec<String>> {
  let mut acc: Vec<Vec<String>> = vec![Vec::new()];
  for _ in 0..k {
    let mut next = Vec::new();
    for combo in &acc {
      for p in parents {
        let mut c = combo.clone();
        c.push(p.clone());
        next.push(c);
      }
    }
    acc = next;
  }
  acc
}

// ---- @extend post-processing -------------------------------------------------

/// Collect every selector text in the CSS tree (rules + at-rule subtrees)
fn collect_tree_selectors(nodes: &[CssNode], out: &mut Vec<String>) {
  for n in nodes {
    match n {
      CssNode::Rule { selector, .. } => out.push(selector.clone()),
      CssNode::AtRule { nodes: sub, .. } => collect_tree_selectors(sub, out),
      _ => {}
    }
  }
}

/// Whether any selector in the compiled tree contains `target`
fn selector_tree_contains(nodes: &[CssNode], target: &str) -> bool {
  let mut sels = Vec::new();
  collect_tree_selectors(nodes, &mut sels);
  sels.iter().any(|s| selector_list_contains(s, target))
}

/// Whether any complex in a comma-separated selector list contains `target`
fn selector_list_contains(list: &str, target: &str) -> bool {
  list
    .split(',')
    .any(|complex| complex_contains(complex, target))
}

/// Whether a complex selector (e.g. `.a .b:hover`) contains `target` —
/// true when some compound in the complex carries all of the target's
/// simple selectors (`.err` matches `.err.foo` and `a.err:hover`)
fn complex_contains(complex: &str, target: &str) -> bool {
  let target = target.trim();
  if target.is_empty() {
    return false;
  }
  let want = simple_tokens(target);
  complex.split_whitespace().any(|compound| {
    let have = simple_tokens(compound);
    want.iter().all(|w| have.contains(w))
  })
}

/// Split a compound selector into its simple-selector tokens:
/// `.class`, `#id`, `%placeholder`, `:pseudo`/`::pseudo` (parens kept),
/// `[attr]`, and a leading type/universal
fn simple_tokens(compound: &str) -> Vec<String> {
  let s = compound.trim();
  let bytes = s.as_bytes();
  let mut tokens = Vec::new();
  let mut i = 0;
  while i < bytes.len() {
    let c = bytes[i] as char;
    match c {
      '.' | '#' | '%' | ':' | '[' | '*' => {
        let start = i;
        i += 1;
        if c == '[' {
          while i < bytes.len() && bytes[i] != b']' {
            i += 1;
          }
          if i < bytes.len() {
            i += 1;
          }
        } else {
          if c == ':' && i < bytes.len() && bytes[i] == b':' {
            i += 1;
          }
          while i < bytes.len() {
            let ch = bytes[i] as char;
            if ch.is_ascii_alphanumeric()
              || ch == '-'
              || ch == '_'
              || (!ch.is_ascii() && !ch.is_whitespace())
            {
              i += 1;
            } else {
              break;
            }
          }
          // pseudo with arguments: `:nth-child(2n+1)`
          if c == ':' && i < bytes.len() && bytes[i] == b'(' {
            let mut depth = 1i32;
            i += 1;
            while i < bytes.len() && depth > 0 {
              match bytes[i] as char {
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
              }
              i += 1;
            }
          }
        }
        tokens.push(s[start..i].to_string());
      }
      _ => {
        // leading type selector: consume the identifier
        let start = i;
        while i < bytes.len() {
          let ch = bytes[i] as char;
          if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            i += 1;
          } else {
            break;
          }
        }
        if i > start {
          tokens.push(s[start..i].to_string());
        } else {
          i += 1;
        }
      }
    }
  }
  tokens
}

/// Apply collected `@extend` records to the final CSS tree
fn apply_extends(
  nodes: &mut Vec<CssNode>,
  records: &[ExtendRecord],
  seen: &[String],
) -> Result<()> {
  if records.is_empty() {
    // Even with no `@extend` anywhere, unextended placeholder rules are
    // omitted from output entirely (reference behavior)
    drop_placeholders(nodes);
    return Ok(());
  }
  // Every extend target must exist among the stylesheet's selectors —
  // the reference compiler errors on unmatched targets
  for r in records {
    if r.optional {
      continue;
    }
    let found = selector_tree_contains(nodes, &r.target)
      || seen.iter().any(|s| selector_list_contains(s, &r.target));
    if !found {
      return Err(Error::syntax(format!(
        "The target selector was not found.\nUse \"@extend {} !optional\" if the extend shouldn't fail.",
        r.target
      )));
    }
  }
  // Transitive extender map: target -> extenders
  let mut extenders: HashMap<String, Vec<String>> = HashMap::new();
  for r in records {
    for ext in r.extender.split(',') {
      let ext = ext.trim().to_string();
      if !ext.is_empty()
        && !extenders
          .entry(r.target.clone())
          .or_default()
          .contains(&ext)
      {
        // The reference prepends each new extender, so the final
        // selector list carries extenders in reverse source order
        // (.a, .b, .c -> ".c, .b, .a").
        extenders.get_mut(&r.target).unwrap().insert(0, ext);
      }
    }
  }
  // Transitive closure (bounded)
  for _ in 0..records.len() + 2 {
    let mut changed = false;
    let targets: Vec<String> = extenders.keys().cloned().collect();
    for t in &targets {
      let current = extenders.get(t).cloned().unwrap_or_default();
      for e in &current {
        if let Some(their) = extenders.get(e).cloned() {
          for x in their {
            let entry = extenders.entry(t.clone()).or_default();
            if !entry.contains(&x) && x != *t {
              entry.push(x);
              changed = true;
            }
          }
        }
      }
    }
    if !changed {
      break;
    }
  }

  rewrite_extends(nodes, &extenders);
  Ok(())
}

fn rewrite_extends(nodes: &mut Vec<CssNode>, extenders: &HashMap<String, Vec<String>>) {
  for node in nodes.iter_mut() {
    match node {
      CssNode::Rule { selector, .. } => {
        let extended = extend_selector_list(selector, extenders);
        *selector = strip_placeholder_complexes(&extended);
      }
      CssNode::AtRule { nodes: sub, .. } => rewrite_extends(sub, extenders),
      _ => {}
    }
  }
  drop_placeholders(nodes);
}

/// Remove complexes made only of `%placeholder` selectors from a list
fn strip_placeholder_complexes(selector: &str) -> String {
  let kept: Vec<&str> = selector
    .split(',')
    .map(|s| s.trim())
    .filter(|complex| {
      !complex.is_empty()
        && !complex
          .split_whitespace()
          .all(|compound| compound.starts_with('%'))
    })
    .collect();
  kept.join(", ")
}

/// Extend every compound in a selector list that contains a target
fn extend_selector_list(selector: &str, extenders: &HashMap<String, Vec<String>>) -> String {
  let mut result: Vec<String> = Vec::new();
  for complex in selector.split(',') {
    let complex = complex.trim();
    let mut variants: Vec<String> = vec![complex.to_string()];
    for (target, exts) in extenders {
      let mut next: Vec<String> = Vec::new();
      for variant in &variants {
        if !next.contains(variant) {
          next.push(variant.clone());
        }
        for ext in exts {
          if let Some(new_sel) = extend_compound(variant, target, ext)
            && !next.contains(&new_sel)
          {
            next.push(new_sel);
          }
        }
      }
      variants = next;
    }
    for v in variants {
      if !result.contains(&v) {
        result.push(v);
      }
    }
  }
  result.join(", ")
}

/// If `complex` (a space-separated compound sequence) contains `target`,
/// return a new complex where the compound containing the target gains the
/// extender's simple selectors
fn extend_compound(complex: &str, target: &str, extender: &str) -> Option<String> {
  let mut changed = false;
  let mut out_parts: Vec<String> = Vec::new();
  for compound in complex.split_whitespace() {
    if let Some(new_compound) = extend_simple(compound, target, extender) {
      changed = true;
      out_parts.push(new_compound);
    } else {
      out_parts.push(compound.to_string());
    }
  }
  if changed {
    Some(out_parts.join(" "))
  } else {
    None
  }
}

/// Extend one simple compound: `.a.b` extended for target `.a` by `.c`
/// yields `.b.c` (the target is replaced by the extender, keeping the
/// compound's other simple selectors)
fn extend_simple(compound: &str, target: &str, extender: &str) -> Option<String> {
  if target.is_empty() || !compound.contains(target) {
    return None;
  }
  let mut out = String::new();
  let mut matched = false;
  let bytes: Vec<char> = compound.chars().collect();
  let tchars: Vec<char> = target.chars().collect();
  let mut i = 0usize;
  while i < bytes.len() {
    if i + tchars.len() <= bytes.len() && bytes[i..i + tchars.len()] == tchars[..] {
      // A target starting with a selector sigil (`.x`, `%ph`, `#id`)
      // is self-anchored: it may directly follow other simple
      // selectors inside a compound (`.btn.primary`)
      let sigil_target = matches!(tchars[0], '.' | '#' | '%' | ':' | '[' | '*' | '&');
      let before_ok = sigil_target
        || i == 0
        || !matches!(
            bytes[i - 1],
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '#' | '%'
        );
      let after_idx = i + tchars.len();
      let after_ok = after_idx >= bytes.len()
        || !matches!(
            bytes[after_idx],
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_'
        );
      if before_ok && after_ok {
        out.push_str(extender);
        matched = true;
        i += tchars.len();
        continue;
      }
    }
    out.push(bytes[i]);
    i += 1;
  }
  if matched { Some(out) } else { None }
}

/// Remove rules whose selectors are only placeholders
fn drop_placeholders(nodes: &mut Vec<CssNode>) {
  for node in nodes.iter_mut() {
    if let CssNode::AtRule { nodes: sub, .. } = node {
      drop_placeholders(sub);
    }
  }
  nodes.retain(|node| match node {
    CssNode::Rule { selector, .. } => {
      let complexes: Vec<&str> = selector
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
      if complexes.is_empty() {
        return false;
      }
      !complexes.iter().all(|c| {
        c.split_whitespace()
          .all(|compound| compound.starts_with('%'))
      })
    }
    _ => true,
  });
}

/// Closest candidate by Levenshtein distance (case-insensitive) — powers
/// "did you mean `$primary`?" suggestions. A candidate sharing a PREFIX
/// with the needle (`primary` <-> `primary-color`) counts as distance 1 so
/// partial names suggest their full spelling in both directions. Ties keep
/// the first candidate; distances beyond ~a third of the needle's length
/// (min 2) don't match.
fn closest_match<'a>(needle: &str, haystack: &[&'a str]) -> Option<&'a str> {
  let n = needle.to_lowercase();
  let limit = (needle.chars().count() / 3).max(2);
  let mut best: Option<(usize, &'a str)> = None;
  for c in haystack {
    let cl = c.to_lowercase();
    let d = if (cl.starts_with(&n) || n.starts_with(&cl)) && cl != n {
      1
    } else {
      levenshtein(&n, &cl)
    };
    if d <= limit && best.map(|(b, _)| d < b).unwrap_or(true) {
      best = Some((d, c));
    }
  }
  best.map(|(_, c)| c)
}

/// Levenshtein edit distance (single-row dynamic programming)
fn levenshtein(a: &str, b: &str) -> usize {
  let a: Vec<char> = a.chars().collect();
  let b: Vec<char> = b.chars().collect();
  let mut prev: Vec<usize> = (0..=b.len()).collect();
  let mut cur = vec![0usize; b.len() + 1];
  for i in 1..=a.len() {
    cur[0] = i;
    for j in 1..=b.len() {
      let cost = usize::from(a[i - 1] != b[j - 1]);
      cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
    }
    std::mem::swap(&mut prev, &mut cur);
  }
  prev[b.len()]
}

#[cfg(test)]
mod tests {
  use super::*;
  use vass_parser::parse_indented;

  fn compile(src: &str) -> String {
    let sheet = parse_indented(src).unwrap();
    let css = evaluate(&sheet).unwrap();
    vass_transpiler::render_expanded(&css)
  }

  #[test]
  fn test_simple_rule() {
    assert_eq!(compile(".a\n  color: red\n"), ".a {\n  color: red;\n}\n");
  }

  #[test]
  fn test_variables_and_arithmetic() {
    let css = compile("$spacing: 1rem\n.a\n  padding: $spacing * 2\n  margin: $spacing + 0.5rem\n");
    assert!(css.contains("padding: 2rem;"));
    assert!(css.contains("margin: 1.5rem;"));
  }

  #[test]
  fn test_nesting_and_parent() {
    let css = compile(".a\n  color: red\n  &:hover\n    color: blue\n  .b\n    color: green\n");
    assert!(css.contains(".a:hover"));
    assert!(css.contains(".a .b"));
    let hover_pos = css.find(".a:hover").unwrap();
    let parent_pos = css.find(".a {").unwrap();
    assert!(parent_pos < hover_pos);
  }

  #[test]
  fn test_undefined_variable() {
    let sheet = parse_indented(".a\n  color: $nope\n").unwrap();
    assert!(evaluate(&sheet).is_err());
  }

  #[test]
  fn test_media_container() {
    let css = compile("@media screen\n  .a\n    color: red\n");
    assert!(css.contains("@media screen {"));
    assert!(css.contains(".a {"));
  }

  #[test]
  fn test_color_function_real_eval() {
    let css = compile("$c: #ff0000\n.a\n  color: darken($c, 10%)\n");
    assert!(css.contains("color: #cc0000;"), "got: {}", css);
  }

  #[test]
  fn test_important_and_default() {
    let css = compile("$x: 1px !default\n$x: 2px !default\n.a\n  width: $x !important\n");
    assert!(css.contains("width: 1px !important;"));
  }

  #[test]
  fn test_if_else() {
    let css = compile(
      "$mode: dark\n.a\n  @if $mode == dark\n    color: white\n  @else\n    color: black\n",
    );
    assert!(css.contains("color: white;"));
    assert!(!css.contains("black"));
  }

  #[test]
  fn test_for_loop() {
    let css = compile("@for $i from 1 through 3\n  .col-#{$i}\n    width: $i * 10px\n");
    assert!(css.contains(".col-1 {"));
    assert!(css.contains(".col-3 {"));
    assert!(css.contains("width: 30px;"));
  }

  #[test]
  fn test_each_map() {
    let css = compile(
      "$colors: (\"a\": red, \"b\": blue)\n@each $name, $c in $colors\n  .c-#{$name}\n    color: $c\n",
    );
    assert!(css.contains(".c-a {"));
    assert!(css.contains("color: red;"));
    assert!(css.contains("color: blue;"));
  }

  #[test]
  fn test_mixin_basic() {
    let css = compile(
      "@mixin flex($dir: row)\n  display: flex\n  flex-direction: $dir\n.a\n  @include flex(column)\n",
    );
    assert!(css.contains("display: flex;"));
    assert!(css.contains("flex-direction: column;"));
    assert!(!css.contains("@mixin"));
    assert!(!css.contains("@include"));
  }

  #[test]
  fn test_mixin_content() {
    let css =
      compile("@mixin wrap\n  padding: 1px\n  @content\n.a\n  @include wrap\n    color: red\n");
    assert!(css.contains("padding: 1px;"));
    assert!(css.contains("color: red;"));
  }

  #[test]
  fn test_function() {
    let css = compile("@function double($n)\n  @return $n * 2\n.a\n  width: double(10px)\n");
    assert!(css.contains("width: 20px;"));
  }

  #[test]
  fn test_function_recursion() {
    let css = compile(
      "@function fib($n)\n  @if $n <= 1\n    @return $n\n  @return fib($n - 1) + fib($n - 2)\n.a\n  width: fib(7)\n",
    );
    assert!(css.contains("width: 13;"), "got: {}", css);
  }

  #[test]
  fn test_extend() {
    let css = compile(".message\n  color: red\n.error\n  @extend .message\n  border: 1px\n");
    assert!(css.contains(".message, .error {"), "got: {}", css);
  }

  #[test]
  fn test_placeholder_extend() {
    let css =
      compile("%base\n  padding: 4px\n.a\n  @extend %base\n  color: red\n.b\n  color: blue\n");
    assert!(!css.contains("%base"));
    assert!(css.contains(".a {"));
    assert!(css.contains("padding: 4px;"));
    assert!(css.contains(".b {"));
    assert!(css.contains("color: blue;"));
  }

  #[test]
  fn test_media_bubbling() {
    let css = compile(".a\n  @media screen\n    color: red\n    .b\n      color: blue\n");
    assert!(css.contains("@media screen {"));
    assert!(css.contains(".a {"));
    assert!(css.contains(".a .b {"));
  }

  #[test]
  fn test_media_merge() {
    let css =
      compile("@media (min-width: 1px)\n  @media (max-width: 2px)\n    .a\n      color: red\n");
    assert!(
      css.contains("@media (min-width: 1px) and (max-width: 2px) {"),
      "got: {}",
      css
    );
  }

  #[test]
  fn test_interpolation() {
    let css = compile("$n: 3\n.a\n  width: #{$n}0px\n.b-#{$n}\n  color: red\n");
    assert!(css.contains("width: 30px;"));
    assert!(css.contains(".b-3 {"));
  }

  #[test]
  fn test_custom_property() {
    let css = compile(".a\n  --x: 1px + 2px\n");
    assert!(css.contains("--x: 1px + 2px;"), "got: {}", css);
  }

  #[test]
  fn test_nested_properties() {
    let css = compile(".a\n  font: 12px\n    family: serif\n    size: 12px\n");
    assert!(css.contains("font: 12px;"));
    assert!(css.contains("font-family: serif;"));
  }

  #[test]
  fn test_null_omitted() {
    let css = compile("$x: null\n.a\n  width: $x\n  color: red\n");
    assert!(!css.contains("width"));
    assert!(css.contains("color: red;"));
  }

  #[test]
  fn test_at_error() {
    let sheet = parse_indented("@error \"nope\"\n").unwrap();
    let err = evaluate(&sheet).unwrap_err();
    assert!(err.message.contains("nope"));
  }

  #[test]
  fn test_math_module() {
    let css = compile("@use \"vass:math\"\n.a\n  width: math.div(10, 2) * 1px\n");
    assert!(css.contains("width: 5px;"), "got: {}", css);
  }

  #[test]
  fn test_namespaced_variable() {
    let css = compile("@use \"vass:math\"\n.a\n  width: math.$pi\n");
    assert!(css.contains("width: 3.1415926536;"), "got: {}", css);
  }

  #[test]
  fn test_comparison_and_logic() {
    let css = compile("$a: 5\n.b\n  x: if($a > 3 and $a < 10, yes, no)\n  y: not false\n");
    assert!(css.contains("x: yes;"));
    assert!(css.contains("y: true;"));
  }
}
