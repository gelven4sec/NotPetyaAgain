use aes::cipher::{generic_array::GenericArray, BlockEncrypt, NewBlockCipher};
use aes::{Aes256, Block, BlockDecrypt};
use core::fmt::Write;
use uefi::table::{Boot, SystemTable};

use crate::file::read_file;

fn read_test_file(st: &SystemTable<Boot>, key_bytes: &[u8; 32]) -> uefi::Result<bool> {
    let key = GenericArray::from_slice(key_bytes);
    let cipher = Aes256::new(key);

    let test_buf = read_file(st, "test")?;

    let mut block = Block::default();
    block.copy_from_slice(&test_buf);

    cipher.decrypt_block(&mut block);

    Ok(block.as_slice() == b"slava ukraini   ")
}

pub fn recover(st: &mut SystemTable<Boot>, key_bytes: &[u8]) -> uefi::Result {
    if key_bytes.len() != 64 {
        st.stdout().write_str("\nWrong key").unwrap();
        return Ok(())
    }

    let mut key = [0u8; 32];
    hex::decode_to_slice(key_bytes, &mut key).unwrap();

    if !read_test_file(st, &key)? {
        st.stdout().write_str("\nWrong key").unwrap();
    } else {
        st.stdout().write_str("\nRight key !").unwrap();
    }
    st.stdout().write_str("\nStart decrypting...").unwrap();

    Ok(())
}
