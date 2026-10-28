// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // With arguments the binary is the `studio` CLI talking to the running app; without, it is the app.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        std::process::exit(gamestudio_lib::cli(args));
    }
    gamestudio_lib::run()
}
