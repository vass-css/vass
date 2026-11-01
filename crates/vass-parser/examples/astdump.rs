fn main() {
  let src = std::env::args().nth(1).expect("usage: astdump <file>");
  match vass_parser::lexer::tokenize(&src) {
    Ok(toks) => {
      for t in &toks {
        println!("  {:?} {:?}", t.kind, t.text);
      }
    }
    Err(e) => println!("  ERR: {}", e),
  }
}
