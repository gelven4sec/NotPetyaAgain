#![no_main]
#![no_std]
#![feature(abi_efiapi)]

extern crate alloc;

use core::fmt::Write;
use uefi::prelude::*;
use uefi::{Char16, ResultExt};
use uefi::proto::console::text::{Color, Key, Output};
use alloc::string::String;

fn init_screen(stdout: &mut Output) {
    stdout.clear().unwrap().unwrap();
    stdout.set_color(Color::Red, Color::Black).unwrap().unwrap();
    stdout.write_str(include_str!("text.txt")).unwrap();
    stdout.write_str("\n> ").unwrap();
}

fn take_input(stdout: &mut Output, char_16: Char16, buffer: &mut String) {
    let char_key = char::from(char_16);
    match char_key {

        // When user press [Enter]
        '\r' => {
            if buffer == "clear" {
                stdout.clear().unwrap().unwrap();
                init_screen(stdout);
            }
            stdout.write_char('\n').unwrap();
            stdout.write_str(buffer.as_str()).unwrap();
            stdout.write_str("\n> ").unwrap();
            buffer.clear();
        }

        // When user press [Backspace]
        '\x08' => {
            if buffer.len() == 0 { return; }
            else {
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

#[entry]
unsafe fn main(_handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&mut system_table).unwrap_success();

    let mut st = system_table.unsafe_clone();
    let st2 = system_table.unsafe_clone();
    let stdin = st.stdin();
    let mut events = [stdin.wait_for_key_event().unsafe_clone()];

    init_screen(system_table.stdout());
    let mut buffer: String = String::from("");

    let boot_services = st2.boot_services();
    loop {
        boot_services.wait_for_event(&mut events).unwrap().unwrap();
        let key = stdin.read_key().unwrap().unwrap();
        match key {
            None => {}
            Some(key) => {
                match key {
                    Key::Printable(key) => {
                        take_input(system_table.stdout(), key, &mut buffer);
                    }
                    Key::Special(_) => {}
                }
            }
        }
    }

    //Status::SUCCESS
}