// Release builds open no console on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod kinds;
#[cfg(test)]
mod kinds_tests;
mod office;
mod placeholder;

use ely_platform::AppId;

fn main() {
    ely_ui::run(AppId::Office, office::open);
}
