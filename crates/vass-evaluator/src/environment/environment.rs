//! Variable/function/mixin scope tracking and the module registry

use std::collections::HashMap;
use std::rc::Rc;

use vass_core::ast::input::expression::Param;
use vass_core::ast::input::statement::Statement;
use vass_core::value::Value;

/// A user-defined function or mixin (owns a clone of its body)
#[derive(Debug, Clone)]
pub struct UserCallable {
  /// Callable name
  pub name: String,
  /// Parameter list
  pub params: Vec<Param>,
  /// Body statements (cloned from the AST at definition time)
  pub body: Vec<Statement>,
  /// Whether this is a mixin (vs a function)
  pub is_mixin: bool,
  /// Variable scopes of the defining environment (module closure):
  /// the body executes with these visible beneath the call scope
  pub env_snapshot: Vec<HashMap<String, Value>>,
}

/// A loaded module: exported members plus its compiled CSS
#[derive(Debug, Clone, Default)]
pub struct Module {
  /// Exported variables
  pub variables: HashMap<String, Value>,
  /// Exported functions
  pub functions: HashMap<String, Rc<UserCallable>>,
  /// Exported mixins
  pub mixins: HashMap<String, Rc<UserCallable>>,
  /// CSS nodes emitted by this module (emitted before the importer's)
  pub nodes: Vec<vass_core::ast::css::CssNode>,
}

/// One lexical scope of variables
#[derive(Debug, Default)]
pub struct Scope {
  vars: HashMap<String, Value>,
  functions: HashMap<String, Rc<UserCallable>>,
  mixins: HashMap<String, Rc<UserCallable>>,
}

/// A stack of scopes; lookups walk from the innermost scope outward.
/// Assignments overwrite the innermost existing definition, or create a
/// local one (VASS semantics).
#[derive(Debug, Default)]
pub struct Environment {
  scopes: Vec<Scope>,
  /// Namespaced modules (`@use "x" as ns`)
  pub modules: HashMap<String, Rc<Module>>,
  /// Whether any module was merged into the global scope with `as *` in
  /// this environment (members are then visible WITHOUT a namespace).
  pub merged_star: bool,
}

impl Environment {
  /// Create an environment with one global scope
  pub fn new() -> Self {
    Environment {
      scopes: vec![Scope::default()],
      modules: HashMap::new(),
      merged_star: false,
    }
  }

  /// Push a new innermost scope
  pub fn push_scope(&mut self) {
    self.scopes.push(Scope::default());
  }

  /// Pop the innermost scope
  pub fn pop_scope(&mut self) {
    if self.scopes.len() > 1 {
      self.scopes.pop();
    }
  }

  /// The global scope (index 0)
  fn global(&mut self) -> &mut Scope {
    &mut self.scopes[0]
  }

  /// Define (or overwrite) a variable. If the name already exists in an
  /// outer scope, the outer value is overwritten; otherwise a new local
  /// binding is created. `global = true` forces the global scope.
  pub fn set(&mut self, name: &str, value: Value, global: bool) {
    if global {
      self.global().vars.insert(name.to_string(), value);
      return;
    }
    for scope in self.scopes.iter_mut().rev() {
      if scope.vars.contains_key(name) {
        scope.vars.insert(name.to_string(), value);
        return;
      }
    }
    if let Some(scope) = self.scopes.last_mut() {
      scope.vars.insert(name.to_string(), value);
    }
  }

  /// Define in the innermost scope unconditionally (parameters, loop vars)
  pub fn define_local(&mut self, name: &str, value: Value) {
    if let Some(scope) = self.scopes.last_mut() {
      scope.vars.insert(name.to_string(), value);
    }
  }

  /// Look a variable up, walking outward through the scopes
  pub fn get(&self, name: &str) -> Option<&Value> {
    for scope in self.scopes.iter().rev() {
      if let Some(v) = scope.vars.get(name) {
        return Some(v);
      }
    }
    None
  }

  /// Whether a variable is defined in any visible scope
  pub fn is_defined(&self, name: &str) -> bool {
    self.get(name).is_some()
  }

  /// Whether a variable is defined in the global scope only
  pub fn is_defined_globally(&self, name: &str) -> bool {
    self.scopes[0].vars.contains_key(name)
  }

  /// Whether a variable is defined in the current (innermost) scope
  pub fn is_defined_locally(&self, name: &str) -> bool {
    self
      .scopes
      .last()
      .is_some_and(|s| s.vars.contains_key(name))
  }

  /// Whether a variable is defined in any non-global (local) scope
  pub fn is_defined_in_local_scopes(&self, name: &str) -> bool {
    self
      .scopes
      .iter()
      .skip(1)
      .any(|s| s.vars.contains_key(name))
  }

  /// Snapshot the global scope's variables (for module exports)
  pub fn global_variables(&self) -> HashMap<String, Value> {
    self.scopes[0].vars.clone()
  }

  /// Snapshot all scopes (for `@content` closures)
  pub fn snapshot(&self) -> Vec<HashMap<String, Value>> {
    self.scopes.iter().map(|s| s.vars.clone()).collect()
  }

  /// Push snapshots back onto the scope stack (content-block closure)
  pub fn push_scopes(&mut self, scopes: &[HashMap<String, Value>]) {
    for vars in scopes {
      self.scopes.push(Scope {
        vars: vars.clone(),
        functions: HashMap::new(),
        mixins: HashMap::new(),
      });
    }
  }

  /// Pop `n` scopes (inverse of `push_scopes`)
  pub fn pop_scopes(&mut self, n: usize) {
    for _ in 0..n {
      if self.scopes.len() > 1 {
        self.scopes.pop();
      }
    }
  }

  /// The global scope's function table (for module exports)
  pub fn global_functions(&self) -> HashMap<String, Rc<UserCallable>> {
    self.scopes[0].functions.clone()
  }

  /// The global scope's mixin table (for module exports)
  pub fn global_mixins(&self) -> HashMap<String, Rc<UserCallable>> {
    self.scopes[0].mixins.clone()
  }

  /// Define a user function or mixin in the global scope, capturing the
  /// current variable scopes as its closure
  pub fn define_callable(
    &mut self,
    decl: &vass_core::ast::input::statement::CallableDecl,
    is_mixin: bool,
  ) {
    let callable = UserCallable {
      name: decl.name.clone(),
      params: decl.params.clone(),
      body: decl.body.clone(),
      is_mixin,
      env_snapshot: self.snapshot(),
    };
    if is_mixin {
      self.define_mixin(&decl.name, callable);
    } else {
      self.define_function(&decl.name, callable);
    }
  }

  // ---- functions -----------------------------------------------------

  /// Define a user function in the global scope (VASS hoists definitions)
  pub fn define_function(&mut self, name: &str, f: UserCallable) {
    self.global().functions.insert(name.to_string(), Rc::new(f));
  }

  /// Look up a user function by plain name
  pub fn get_function(&self, name: &str) -> Option<Rc<UserCallable>> {
    for scope in self.scopes.iter().rev() {
      if let Some(f) = scope.functions.get(name) {
        return Some(f.clone());
      }
    }
    None
  }

  /// Whether a user function exists
  pub fn has_function(&self, name: &str) -> bool {
    self.get_function(name).is_some()
  }

  // ---- mixins --------------------------------------------------------

  /// Define a user mixin in the global scope
  pub fn define_mixin(&mut self, name: &str, m: UserCallable) {
    self.global().mixins.insert(name.to_string(), Rc::new(m));
  }

  /// Look up a user mixin by plain name
  pub fn get_mixin(&self, name: &str) -> Option<Rc<UserCallable>> {
    for scope in self.scopes.iter().rev() {
      if let Some(m) = scope.mixins.get(name) {
        return Some(m.clone());
      }
    }
    None
  }

  /// Whether a user mixin exists
  pub fn has_mixin(&self, name: &str) -> bool {
    self.get_mixin(name).is_some()
  }

  // ---- modules -------------------------------------------------------

  /// Register a module under a namespace
  pub fn register_module(&mut self, namespace: &str, module: Module) {
    self.modules.insert(namespace.to_string(), Rc::new(module));
  }

  /// Merge a module's members into the global scope (`@use ... as *`)
  pub fn merge_module_globals(&mut self, module: &Module) {
    for (k, v) in &module.variables {
      self.scopes[0].vars.insert(k.clone(), v.clone());
    }
    for (k, v) in &module.functions {
      self.scopes[0].functions.insert(k.clone(), v.clone());
    }
    for (k, v) in &module.mixins {
      self.scopes[0].mixins.insert(k.clone(), v.clone());
    }
    self.merged_star = true;
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use vass_core::value::Number;

  #[test]
  fn test_scoping() {
    let mut env = Environment::new();
    env.set("x", Value::Number(Number::unitless(1.0)), false);
    env.push_scope();
    env.set("y", Value::Number(Number::unitless(2.0)), false);
    assert!(env.is_defined("x"));
    assert!(env.is_defined("y"));
    env.pop_scope();
    assert!(env.is_defined("x"));
    assert!(!env.is_defined("y"));
  }

  #[test]
  fn test_shadowing_writes_outer() {
    let mut env = Environment::new();
    env.set("x", Value::Number(Number::unitless(1.0)), false);
    env.push_scope();
    env.set("x", Value::Number(Number::unitless(2.0)), false);
    assert_eq!(env.get("x"), Some(&Value::Number(Number::unitless(2.0))));
    env.pop_scope();
    // VASS semantics: the outer binding was overwritten
    assert_eq!(env.get("x"), Some(&Value::Number(Number::unitless(2.0))));
  }

  #[test]
  fn test_new_var_is_local() {
    let mut env = Environment::new();
    env.push_scope();
    env.set("x", Value::Number(Number::unitless(1.0)), false);
    env.pop_scope();
    assert!(!env.is_defined("x"));
  }

  #[test]
  fn test_global_flag() {
    let mut env = Environment::new();
    env.push_scope();
    env.set("x", Value::Number(Number::unitless(5.0)), true);
    env.pop_scope();
    assert!(env.is_defined("x"));
  }

  #[test]
  fn test_define_local_overwrites() {
    let mut env = Environment::new();
    env.set("x", Value::Number(Number::unitless(1.0)), false);
    env.push_scope();
    env.define_local("x", Value::Number(Number::unitless(9.0)));
    assert_eq!(env.get("x"), Some(&Value::Number(Number::unitless(9.0))));
    env.pop_scope();
    assert_eq!(env.get("x"), Some(&Value::Number(Number::unitless(1.0))));
  }
}
