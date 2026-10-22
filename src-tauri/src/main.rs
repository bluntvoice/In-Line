#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if in_line_lib::mcp::build_identity::print_if_requested("in-line") {
        return;
    }
    in_line_lib::run();
}
