// Release builds open no console on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod files;
mod home;
mod kinds;
#[cfg(test)]
mod kinds_tests;
mod office;
mod placeholder;

use ely_platform::AppId;

fn main() {
    // The taskbar and the jump list need a name of their own for this
    // program, or the window is filed under the shell's idea of it.
    identity();
    ely_ui::run(AppId::Office, office::open);
}

/// Gives the process its own identity in the taskbar.
#[cfg(windows)]
fn identity() {
    use windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
    let name: Vec<u16> = "MPelyoffice.MPelyoffice".encode_utf16().collect();
    unsafe {
        SetCurrentProcessExplicitAppUserModelID(name.as_ptr());
    }
}

#[cfg(not(windows))]
fn identity() {}
