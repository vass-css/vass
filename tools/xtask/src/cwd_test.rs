#[test]
fn test_cwd() {
    let cwd = std::env::current_dir().unwrap();
    eprintln!("TEST CWD: {:?}", cwd);
}
