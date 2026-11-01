// Prevents an extra console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("run") {
        std::process::exit(keyhandoff_lib::run_command(&args[1..]));
    }
    keyhandoff_lib::run()
}
