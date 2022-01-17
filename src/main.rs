#![no_main]
#![no_std]
#![feature(abi_efiapi)]

use core::fmt::Write;
use uefi::prelude::*;
use uefi::{Char16, ResultExt};
use uefi::proto::console::text::{Color, Key, Output};

fn init_screen(stdout: &mut Output) {
    stdout.clear().unwrap().unwrap();
    stdout.set_color(Color::Red, Color::Black).unwrap().unwrap();
    stdout.write_str(include_str!("text.txt")).unwrap();
    stdout.write_str("\n> ").unwrap();
}

fn write_char(stdout: &mut Output, char_key: Char16) {
    stdout.write_char(char::from(char_key)).unwrap();
}

#[entry]
unsafe fn main(_handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&mut system_table).unwrap_success();

    let mut st = system_table.unsafe_clone();
    let stdin = st.stdin();
    //let mut events = [stdin.wait_for_key_event().unsafe_clone()];

    init_screen(system_table.stdout());

    //let boot_services = st.boot_services();
    loop {
        //boot_services.wait_for_event(&mut events).unwrap();
        let key = stdin.read_key().unwrap().unwrap();
        match key {
            None => {}
            Some(key_c) => {
                match key_c {
                    Key::Printable(char_key) => {
                        write_char(system_table.stdout(), char_key)
                    }
                    Key::Special(_) => {}
                }
            }
        }
    }

    //Status::SUCCESS
}