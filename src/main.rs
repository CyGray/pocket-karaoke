#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod gui;

#[cfg(windows)]
fn main() {
    if let Err(error) = gui::run() {
        eprintln!("Pocket Karaoke UI error: {error}");
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Pocket Karaoke's Slint UI runs on Windows. Use the GitHub release EXE.");
}
