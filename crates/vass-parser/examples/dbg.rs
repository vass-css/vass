fn main() {
  let src = "@media screen\n  .a\n    color: red\n";
  match vass_parser::parse(src, vass_core::ast::input::Syntax::Indented) {
    Ok(sheet) => {
      for s in &sheet.body {
        println!("{:#?}", s);
      }
    }
    Err(e) => println!("ERR: {}", e),
  }
}
