#![no_main]
#![no_std]
#![feature(abi_efiapi)]

use core::fmt::Write;
use uefi::prelude::*;
use uefi::{Event, ResultExt};
use uefi::proto::console::text::{Color, Input, Output};


fn write_to_screen(stdout: &mut Output) {
    stdout.set_color(Color::Red, Color::Black);
    stdout.write_str(include_str!("text.txt"));
    stdout.write_str("\n> ");
}

unsafe fn get_key_event(stdin: &mut Input) -> Event {
    stdin.wait_for_key_event().unsafe_clone()
}

#[entry]
unsafe fn main(_handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&mut system_table).unwrap_success();

    write_to_screen(system_table.stdout());
    let mut events = [get_key_event(system_table.stdin())];

    let boot_services = system_table.boot_services();

    loop {
        boot_services.wait_for_event(&mut events);
    }

    //Status::SUCCESS
}