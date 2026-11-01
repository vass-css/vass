//! Environment module
//!
//! Lexical scoping for variables during evaluation.

#[allow(clippy::module_inception)]
mod environment;

pub use environment::{Environment, Module, UserCallable};
