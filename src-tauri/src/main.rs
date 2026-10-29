#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if in_line_lib::mcp::build_identity::print_if_requested("in-line") {
        return;
    }
    if !in_line_lib::install_gate::ready() {
        eprintln!("{}", in_line_lib::install_gate::MESSAGE);
        #[cfg(windows)]
        unsafe {
            let message: Vec<u16> = in_line_lib::install_gate::MESSAGE
                .encode_utf16()
                .chain(Some(0))
                .collect();
            let title: Vec<u16> = "In Line".encode_utf16().chain(Some(0)).collect();
            windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW(
                std::ptr::null_mut(),
                message.as_ptr(),
                title.as_ptr(),
                0x10,
            );
        }
        std::process::exit(78);
    }
    in_line_lib::run();
}
