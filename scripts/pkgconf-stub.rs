//! A stand-in for pkg-config, for the LibreOffice build on Windows.
//!
//! LibreOffice's autogen looks for a program called `pkgconf-2.4.3.exe` that
//! reports Windows-style paths; it uses it when meson builds harfbuzz, and
//! harfbuzz's freetype support is optional. This program answers the version
//! question and says that no package is there, which is the same answer a
//! machine without a system freetype would give.
//!
//! Build it with:
//!   rustc -O -o pkgconf-2.4.3.exe scripts/pkgconf-stub.rs
//! and put it where the build finds it (the PATH, or LODE_HOME/opt/bin).

use std::io::Write;

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.iter().any(|argument| argument == "--version") {
        println!("2.4.3");
        return;
    }
    if arguments.iter().any(|argument| argument == "--help") {
        println!("pkgconf 2.4.3 (a stand-in that finds no packages)");
        return;
    }
    // A package was asked for and there is none: pkg-config says so by
    // exiting with one and printing nothing.
    let _ = std::io::stdout().flush();
    std::process::exit(1);
}
