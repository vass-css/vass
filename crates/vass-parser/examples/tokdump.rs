fn main() {
  for src in [".a { a: [h]; }", ".a { a: [m]; }", ".a { a: [u]; }"] {
    println!("=== {}", src);
    match vass_parser::lexer::tokenize(src) {
      Ok(toks) => {
        for t in &toks {
          println!("  {:?} {:?}", t.kind, t.text);
        }
      }
      Err(e) => println!("  ERR: {}", e),
    }
  }
}
