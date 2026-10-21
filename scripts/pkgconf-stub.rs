//! A stand-in for pkg-config, for the LibreOffice build on Windows.
//!
//! LibreOffice's autogen looks for a program called `pkgconf-2.4.3.exe` that
//! reports Windows-style paths; it uses it when meson builds harfbuzz, and
//! harfbuzz's freetype support is optional. This program answers the version
//! question and says that no package is there, which is the same answer a
//! machine without a system freetype would give.
//!
//! The packages that LibreOffice builds itself are answered as being there,
//! with no flags: the build gives the folders and the libraries to the
//! compiler itself, and the question is only a gate the package's own build
//! has to pass.
//!
//! Build it with:
//!   rustc -O -o pkgconf-2.4.3.exe scripts/pkgconf-stub.rs
//! and put it where the build finds it (the PATH, or LODE_HOME/opt/bin).

use std::io::Write;

/// The packages the build has of its own.
const KNOWN: &[&str] = &[
    "libxml-2.0",
    "libxslt",
    "libicu-uc",
    "libicu-i18n",
    "gobject-2.0",
    "glib-2.0",
    "gio-2.0",
    "gmodule-2.0",
    "zlib",
    "libpng",
    "libpng16",
    "freetype2",
    "harfbuzz",
    "cairo",
    "cairo-gobject",
    "libcurl",
    "openssl",
    "libexpat",
    "pixman-1",
    "fontconfig",
    "libxml2",
];

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.iter().any(|argument| argument == "--version") {
        println!("2.4.3");
        return;
    }
    if arguments.iter().any(|argument| argument == "--help") {
        println!("pkgconf 2.4.3 (a stand-in for the packages the build has of its own)");
        return;
    }
    if arguments
        .iter()
        .any(|argument| argument.starts_with("--atleast-pkgconfig-version"))
    {
        // The version question is always answered yes: the build asks it of
        // every package, and the answer decides whether the package's own
        // build goes on.
        println!("2.4.3");
        return;
    }
    if arguments.iter().any(|argument| argument == "--exists") {
        // The question of a package: the ones the build has are there, the
        // others are not.
        let asked = arguments
            .iter()
            .skip_while(|argument| *argument != "--exists")
            .skip(1)
            .flat_map(|argument| argument.split(','))
            .map(|name| name.trim())
            .filter(|name| !name.is_empty())
            .any(|name| {
                let name = name.split(' ').next().unwrap_or(name);
                let name = name.split('>').next().unwrap_or(name);
                let name = name.split('=').next().unwrap_or(name);
                KNOWN.contains(&name)
            });
        let _ = std::io::stdout().flush();
        std::process::exit(if asked { 0 } else { 1 });
    }
    if arguments
        .iter()
        .any(|argument| argument == "--cflags" || argument == "--libs")
    {
        // The flags are given by the build itself, so there are none here.
        println!();
        return;
    }
    // A package was asked for and there is none: pkg-config says so by
    // exiting with one and printing nothing.
    let _ = std::io::stdout().flush();
    std::process::exit(1);
}
