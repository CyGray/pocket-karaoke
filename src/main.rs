#[cfg(any(windows, test))]
mod audio;
#[cfg(any(windows, test))]
mod protocol;

#[cfg(windows)]
mod desktop;

#[cfg(windows)]
fn main() {
    if let Err(error) = desktop::run() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("The audio receiver currently targets Windows. Build on Windows, or download the Windows release from GitHub.");
    std::process::exit(1);
}
