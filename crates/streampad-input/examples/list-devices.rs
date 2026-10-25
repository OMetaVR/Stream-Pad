//! Prints this machine's keyboards and whether StreamPad could grab them. Read-only: devices are opened for
//! querying, never grabbed.
//!
//! `cargo run -p streampad-input --example list-devices [toggle-key-name]`

use streampad_core::combo::key_code;
use streampad_core::keys::KEY_SCROLLLOCK;
use streampad_input::{DeviceFilter, list_devices};

fn main() {
    let toggle_key = match std::env::args().nth(1) {
        Some(name) => key_code(&name).unwrap_or_else(|| {
            eprintln!("unknown key name {name:?}");
            std::process::exit(2);
        }),
        None => KEY_SCROLLLOCK,
    };
    let devices = list_devices(&DeviceFilter { auto: true, ..Default::default() }, toggle_key);
    if devices.is_empty() {
        println!("No keyboards found.");
        return;
    }
    let yes_no = |flag: bool| if flag { "yes" } else { "no" };
    println!("{:<20} {:<9} {:<7} ID", "DEVICE", "ELIGIBLE", "NUMPAD");
    for device in &devices {
        println!("{:<20} {:<9} {:<7} {}", device.path, yes_no(device.eligible), yes_no(device.has_numpad), device.id);
        if let Some(phys) = &device.phys {
            println!("{:<20} phys {phys}", "");
        }
        if let Some(error) = &device.error {
            println!("{:<20} error: {error}", "");
        }
    }
}
