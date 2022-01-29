#![no_main]
#![no_std]
#![feature(abi_efiapi)]

extern crate alloc;

use core::fmt::Write;
use uefi::prelude::*;
use uefi::{Char16, Event, ResultExt};
use uefi::proto::console::text::{Color, Key, Output};
use alloc::string::String;
use core::alloc::Layout;
use core::str;
use uefi::exts::allocate_buffer;
use uefi::proto::media::file::{File, FileAttribute, FileInfo, FileMode};
use uefi::proto::media::fs::SimpleFileSystem;

fn init_screen(stdout: &mut Output) {
    stdout.clear().unwrap().unwrap();
    stdout.enable_cursor(true).unwrap_success();
    stdout.set_color(Color::Red, Color::Black).unwrap_success();
    stdout.write_str(include_str!("text.txt")).unwrap();
    stdout.write_str("\n> ").unwrap();
}

fn take_input(system_table: &mut SystemTable<Boot>, char_16: Char16, buffer: &mut String) {
    let st = unsafe {system_table.unsafe_clone()};
    let stdout = system_table.stdout();
    let char_key = char::from(char_16);
    match char_key {

        // When user press [Enter]
        '\r' => {
            if buffer == "clear" {
                stdout.clear().unwrap_success();
                init_screen(stdout);
                buffer.clear();
            } else if buffer == "test" {
                let fs = st
                    .boot_services()
                    .locate_protocol::<SimpleFileSystem>()
                    .unwrap_success();

                let fs = unsafe { &mut *fs.get() };
                let mut root = fs.open_volume().unwrap().unwrap();
                let text_file_handle = root
                    .open("test.txt", FileMode::Read, FileAttribute::empty())
                    .expect("Failed to load kernel (expected file named `test.txt`)")
                    .unwrap();
                let mut text_file = match text_file_handle.into_type().unwrap().unwrap() {
                    uefi::proto::media::file::FileType::Regular(f) => f,
                    uefi::proto::media::file::FileType::Dir(_) => panic!("Not expecting a directory"),
                };

                let mut buf = [0; 500];
                let text_info: &mut FileInfo = text_file.get_info(&mut buf).unwrap().unwrap();
                let text_size = usize::try_from(text_info.file_size()).unwrap();

                let buf_layout = Layout::array::<u8>(text_size).unwrap();
                let mut buf = allocate_buffer(buf_layout);

                text_file.read(&mut buf).unwrap_success();

                text_file.close();

                let text_content = unsafe { str::from_utf8_unchecked(&buf)};

                stdout.write_char('\n').unwrap();
                stdout.write_str(text_content).unwrap();
                stdout.write_str("\n> ").unwrap();
                buffer.clear();
            }
            else {
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

#[entry]
fn main(_handle: Handle, mut st: SystemTable<Boot>) -> Status {
    uefi_services::init(&mut st).unwrap_success();

    init_screen(st.stdout());
    let mut buffer: String = String::from("");

    let mut key_event = unsafe { [st.stdin().wait_for_key_event().unsafe_clone()] };

    loop {
        wait_for_input(st.boot_services(), &mut key_event);
        match st.stdin().read_key().unwrap_success() {
            None => {}
            Some(key) => {
                match key {
                    Key::Printable(key) => {
                        take_input(&mut st, key, &mut buffer);
                    }
                    Key::Special(_) => {}
                }
            }
        }
    }

    //Status::SUCCESS
}
