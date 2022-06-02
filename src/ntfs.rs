use alloc::vec;
use alloc::vec::Vec;
use core::ops::Range;
use uefi::proto::media::block::BlockIO;
use uefi::{Error, Status};

pub const OEM_ID: &[u8; 8] = b"NTFS    ";

pub fn read_mft_entry(
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

pub fn get_data_runs(mft_entry_buf: &[u8]) -> uefi::Result<Vec<Range<u64>>> {
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
                    (u32::from_ne_bytes(data_run_first.try_into().unwrap()) as u64) * 8;

                ranges.push(data_run_first..data_run_first + data_run_size);
                data_run_offset += 5;
            }

            0x32 => {
                let data_run_size = (u16::from_ne_bytes(
                    mft_entry_buf[data_run_offset + 1..data_run_offset + 3]
                        .try_into()
                        .unwrap(),
                ) as u64)
                    * 8;

                let mut data_run_first =
                    mft_entry_buf[data_run_offset + 3..data_run_offset + 6].to_vec();
                data_run_first.push(0);
                let data_run_first =
                    (u32::from_ne_bytes(data_run_first.try_into().unwrap()) as u64) * 8;

                ranges.push(data_run_first..data_run_first + data_run_size);
                data_run_offset += 6;
            }

            0x33 => {
                let mut data_run_size =
                    mft_entry_buf[data_run_offset + 1..data_run_offset + 4].to_vec();
                data_run_size.push(0);
                let data_run_size =
                    (u32::from_ne_bytes(data_run_size.try_into().unwrap()) as u64) * 8;
                let mut data_run_first =
                    mft_entry_buf[data_run_offset + 4..data_run_offset + 7].to_vec();
                data_run_first.push(0);
                let data_run_first =
                    (u32::from_ne_bytes(data_run_first.try_into().unwrap()) as u64) * 8;
                ranges.push(data_run_first..data_run_first + data_run_size);
                data_run_offset += 7;
            }

            0x42 => {
                let data_run_size = (u16::from_ne_bytes(
                    mft_entry_buf[data_run_offset + 1..data_run_offset + 3]
                        .try_into()
                        .unwrap(),
                ) as u64)
                    * 8;
                let data_run_first = (u32::from_ne_bytes(
                    mft_entry_buf[data_run_offset + 3..data_run_offset + 7]
                        .try_into()
                        .unwrap(),
                ) as u64)
                    * 8;
                ranges.push(data_run_first..data_run_first + data_run_size);
                data_run_offset += 7;
            }

            _ => break,
        }
    }

    Ok(ranges)
}
