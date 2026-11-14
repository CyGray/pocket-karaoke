#[cfg(windows)]
fn main() {
    if let Err(error) = pocket_karaoke::desktop::run() {
        eprintln!("Pocket Karaoke CLI error: {error}");
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Pocket Karaoke's audio receiver runs on Windows.");
}
