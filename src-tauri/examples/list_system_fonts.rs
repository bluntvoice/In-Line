// Read-only native inventory used by the isolated browser regression harness.
fn main() {
    match in_line_lib::fonts::system_fonts() {
        Ok(fonts) => println!("{}", serde_json::to_string(&fonts).unwrap()),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
