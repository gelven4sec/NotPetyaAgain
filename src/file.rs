use alloc::boxed::Box;
use alloc::vec::Vec;
use core::alloc::Layout;
use uefi::exts::allocate_buffer;
use uefi::prelude::{Boot, SystemTable};
use uefi::proto::media::file::{Directory, File, FileAttribute, FileInfo, FileMode};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::ResultExt;

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

pub fn read_file(st: &SystemTable<Boot>, filepath: &str) -> Result<Box<[u8]>, ()> {
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