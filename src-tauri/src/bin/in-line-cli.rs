//! Windows console entry: stdout belongs exclusively to the requested output.
#[tokio::main]
async fn main() {
    let code = in_line_lib::cli::run(std::env::args_os().skip(1)).await;
    std::process::exit(code);
}
