//! Sass statements (rules, declarations, variables, mixins, control flow,
//! modules)

use crate::ast::vass::expression::{CallArg, Expr, Param};
use crate::ast::vass::Syntax;

/// A parsed stylesheet
#[derive(Debug, Clone, PartialEq)]
pub struct Stylesheet {
    /// Top-level statements
    pub body: Vec<Statement>,
    /// The syntax this stylesheet was parsed from
    pub syntax: Syntax,
}

/// A function or mixin definition
#[derive(Debug, Clone, PartialEq)]
pub struct CallableDecl {
    /// Name without `@`/parens
    pub name: String,
    /// Parameter list
    pub params: Vec<Param>,
    /// Body statements
    pub body: Vec<Statement>,
}

/// An `@if`/`@else if`/`@else` chain
#[derive(Debug, Clone, PartialEq)]
pub struct IfChain {
    /// Ordered `(condition, body)` clauses; the first is `@if`
    pub clauses: Vec<(Expr, Vec<Statement>)>,
    /// `@else` body, if any
    pub else_body: Option<Vec<Statement>>,
}

/// A `@use`/`@forward` rule
#[derive(Debug, Clone, PartialEq)]
pub struct UseRule {
    /// Module URL (`"sass:math"`, `"variables"`, `"src/components/_button"`)
    pub url: String,
    /// Explicit namespace (`as ns`); `None` = default, `Some("")` = `as *`
    pub namespace: Option<String>,
    /// Configured variables (`with ($a: 1, ...)`)
    pub with: Vec<(String, Expr)>,
    /// `@forward` show list (member names)
    pub show: Vec<String>,
    /// `@forward` hide list (member names)
    pub hide: Vec<String>,
    /// `@forward` prefix (`as prefix-*`)
    pub prefix: Option<String>,
}

/// An `@import` target: either a Sass file import or a plain CSS import
#[derive(Debug, Clone, PartialEq)]
pub struct ImportTarget {
    /// The URL string (quotes stripped for Sass imports)
    pub url: String,
    /// Whether the URL was written quoted
    pub quoted: bool,
    /// Media query suffix for plain CSS imports (`screen and (...)`)
    pub media: Option<String>,
    /// Whether this target must pass through as plain CSS
    /// (`.css` files, `url(...)`, `http(s)://`, ...)
    pub plain_css: bool,
}

/// A statement in a Sass stylesheet
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    /// `$name: value !default !global;`
    VariableDecl {
        /// Variable name without `$`
        name: String,
        /// The value expression
        value: Expr,
        /// Whether `!default` was given
        guarded: bool,
        /// Whether `!global` was given
        global: bool,
    },
    /// `property: value !important;`
    Declaration {
        /// Property name (may contain interpolation, evaluated later)
        prop: String,
        /// The value expression; `None` for value-less nested properties
        value: Option<Expr>,
        /// Whether `!important` was given
        important: bool,
        /// Nested property children (`font: 12px { family: serif }`)
        children: Vec<Statement>,
        /// Whether this is a custom property (`--foo: value`)
        custom: bool,
        /// Raw custom property source text (interpolation applied at eval)
        custom_raw: Option<String>,
    },
    /// A style rule (selector + nested statements)
    StyleRule {
        /// Raw selector text (may contain interpolation)
        selector: String,
        /// Nested statements
        body: Vec<Statement>,
    },
    /// A block at-rule (`@media`, `@supports`, `@keyframes`, `@font-face`, ...)
    AtRule {
        /// At-rule name without `@`
        name: String,
        /// Raw parameter text after the name (may contain interpolation)
        params: String,
        /// Body statements
        body: Vec<Statement>,
    },
    /// A statement at-rule passed through (`@charset`, CSS `@import`)
    AtRuleStmt {
        /// Raw text including the leading `@`
        raw: String,
    },
    /// `@mixin name($a, $b: 1, $rest...) { ... }`
    MixinDecl(CallableDecl),
    /// `@function name($a) { @return ...; }`
    FunctionDecl(CallableDecl),
    /// `@include name(args...) { content }`
    Include {
        /// Mixin name
        name: String,
        /// Arguments
        args: Vec<CallArg>,
        /// `@content` block, if given
        content: Option<Vec<Statement>>,
        /// `using ($x, $y)` bound content variables
        using: Vec<Param>,
    },
    /// `@content(args)` inside a mixin
    Content {
        /// Arguments passed to the content block
        args: Vec<CallArg>,
    },
    /// `@if` / `@else if` / `@else`
    If(IfChain),
    /// `@each $a, $b in $list { ... }`
    Each {
        /// Loop variable names (without `$`)
        vars: Vec<String>,
        /// The list expression
        list: Expr,
        /// Loop body
        body: Vec<Statement>,
    },
    /// `@for $i from a through b` / `@for $i from a to b`
    For {
        /// Loop variable name
        var: String,
        /// Start expression
        from: Expr,
        /// End expression
        to: Expr,
        /// `true` for `through` (inclusive), `false` for `to` (exclusive)
        inclusive: bool,
        /// Loop body
        body: Vec<Statement>,
    },
    /// `@while cond { ... }`
    While {
        /// Condition expression
        cond: Expr,
        /// Loop body
        body: Vec<Statement>,
    },
    /// `@return value;` inside a function
    Return {
        /// The returned expression
        value: Expr,
    },
    /// `@extend selector !optional;`
    Extend {
        /// The target selector text
        target: String,
        /// Whether `!optional` was given
        optional: bool,
    },
    /// `@at-root (query) { ... }`
    AtRoot {
        /// Optional query text (e.g. `(without: media)`)
        query: Option<String>,
        /// Body statements
        body: Vec<Statement>,
    },
    /// `@debug expr;`
    Debug {
        /// The expression to print
        value: Expr,
    },
    /// `@warn expr;`
    Warn {
        /// The expression to print
        value: Expr,
    },
    /// `@error expr;`
    Error {
        /// The expression to raise
        value: Expr,
    },
    /// `@use "url" as ns with (...)`
    Use(UseRule),
    /// `@forward "url" as prefix-* show ...`
    Forward(UseRule),
    /// `@import "a", "b" screen;`
    Import(Vec<ImportTarget>),
    /// A loud comment (`/* ... */`), preserved in output
    Comment {
        /// Raw comment text including delimiters
        text: String,
    },
}
