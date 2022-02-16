use alloc::vec;
// Includes structs and APIs for handing of the GPT partition table format
use uefi::Guid;
use uefi::proto::media::block::BlockIO;
use crate::alloc::vec::Vec;
use core::convert::TryInto;

const EFI_SIG: [u8; 8] = *b"EFI PART";

/// define our GPT Partition Table header
#[derive(Copy,Clone,PartialEq)]
pub struct GPTHeader {
    // [0..8] -> EFI SIG
    revision:           u32, // [8..12]
    header_sz:          u32, // [12..16]
    crc32:              u32, // [16..20] note to check this, assume this is filled with zero
    // [20..24] -> RESERVED (must be zeroes)
    curr_lba:           u64, // [24..32]
    backup_lba:         u64, // [32..40]
    first_lba:          u64, // [40..48]
    last_lba:           u64, // [48..56]
    guid:               Guid, // [56..72]
    lba_part_entries:   u64, // [72, 80]
    num_partitions:     u32, // [80..84]
    pub part_size:          u32, // [84..88]
    part_crc32:         u32 // [88..92]
    // [92] -> RESERVED, rest are zeroes
}


/// define our GPT Partition Entry struct
#[derive(Copy,Clone,PartialEq)]
pub struct GPTPartition {
    pub part_type_guid:     Guid, // [0..16] (See below for list of type GUIDs)
    // https://en.wikipedia.org/wiki/GUID_Partition_Table#Partition_type_GUIDs
    pub part_guid:          Guid, // [16..32]
    pub first_lba:          u64, // [32..40]
    pub last_lba:           u64, // [40..48]
    attr_flags:         u64, // [48..56]
    part_name:          [u8; 72] // [56..128] note this is stored using UTF-16 LE encoding...
}

/// define out GPT struct, which will take an EFI disk to parse
pub struct GPTDisk {
    media_id:   u32,
    blocksize:  u32,
    header:     GPTHeader,
    partitions: Vec<GPTPartition>
}


/// helper function to parse GUIDs from raw bytes
pub fn bytes_to_guid(bytes: [u8; 16]) -> Guid {
    // convert the bytes to usable values
    let time_low = u32::from_ne_bytes(bytes[0..4].try_into().unwrap());
    let time_mid = u16::from_ne_bytes(bytes[4..6].try_into().unwrap());
    let time_high = u16::from_ne_bytes(bytes[6..8].try_into().unwrap());
    let mut clock_seq_data = bytes[8..10].to_vec();
    clock_seq_data.reverse();
    let clock_seq = u16::from_ne_bytes(clock_seq_data.try_into().unwrap());
    let mut node_data = bytes[10..16].to_vec();
    node_data.reverse();
    node_data.push(0);
    node_data.push(0);
    let node = u64::from_ne_bytes(node_data.try_into().unwrap());

    // generate the GUID structure
    Guid::from_values(
        time_low,
        time_mid,
        time_high,
        clock_seq,
        node
    )
}


///////////////////////// GPTHEADER IMPL /////////////////////////////////
impl GPTHeader{
    /// creates a new GPTHeader struct from raw bytes
    pub fn new(sector: [u8; 512]) -> Self {
        // fetch all of the values we need
        let revision            = u32::from_ne_bytes(sector[8..12].try_into().unwrap());
        let header_sz           = u32::from_ne_bytes(sector[12..16].try_into().unwrap());
        let crc32               = u32::from_ne_bytes(sector[16..20].try_into().unwrap());
        let curr_lba            = u64::from_ne_bytes(sector[24..32].try_into().unwrap());
        let backup_lba          = u64::from_ne_bytes(sector[32..40].try_into().unwrap());
        let first_lba           = u64::from_ne_bytes(sector[40..48].try_into().unwrap());
        let last_lba            = u64::from_ne_bytes(sector[48..56].try_into().unwrap());
        let lba_part_entries    = u64::from_ne_bytes(sector[72..80].try_into().unwrap());
        let num_partitions      = u32::from_ne_bytes(sector[80..84].try_into().unwrap());
        let part_size           = u32::from_ne_bytes(sector[84..88].try_into().unwrap());
        let part_crc32          = u32::from_ne_bytes(sector[88..92].try_into().unwrap());
        let guid                = bytes_to_guid(sector[56..72].try_into().unwrap());

        // finally we can create the structure
        GPTHeader{
            revision,
            header_sz,
            crc32,
            curr_lba,
            backup_lba,
            first_lba,
            last_lba,
            guid,
            lba_part_entries,
            num_partitions,
            part_size,
            part_crc32
        }
    }

    /// returns the number of partitions available
    pub fn num_partitions(&self) -> u32 {
        self.num_partitions
    }
}

////////////////////////// GPTPARTITION IMPL //////////////////////////////
impl GPTPartition {
    /// creates a new GPTPartition from raw bytes
    pub fn new(chunk: [u8; 128]) -> Self {
        // get the various guids
        let part_type_guid          = bytes_to_guid(chunk[0..16].try_into().unwrap());
        let part_guid               = bytes_to_guid(chunk[16..32].try_into().unwrap());

        // get the various lba and flag things
        let first_lba               = u64::from_ne_bytes(chunk[32..40].try_into().unwrap());
        let last_lba                = u64::from_ne_bytes(chunk[40..48].try_into().unwrap());
        let attr_flags              = u64::from_ne_bytes(chunk[48..56].try_into().unwrap());

        // get the partition name
        let part_name: [u8; 72]     = chunk[56..].try_into().unwrap();

        // return the structure
        GPTPartition {
            part_type_guid,
            part_guid,
            first_lba,
            last_lba,
            attr_flags,
            part_name
        }
    }
}


/// define our functions for the GPT struct so we can use it later on
impl GPTDisk {
    /// creates a new GPT structure
    pub fn new(blk: &BlockIO, media_id: u32, blk_size: u32, mut buf: &mut [u8]) -> Self {

        blk.read_blocks(media_id, 1, &mut buf)
            .expect("Failed to read bytes")
            .unwrap();

        // parse the GPT header
        let header = GPTHeader::new(buf.try_into().unwrap());
        let mut partitions: Vec<GPTPartition> = Vec::new();

        // find the number of partitions and where they are located
        let num_part    = header.num_partitions;
        let array_lba   = header.lba_part_entries;
        let read_total  = header.part_size * num_part;
        let read_total  = read_total + read_total % blk_size;

        // find the device we are operating on, and get the UEFI BlockIO protocol
        let mut buf: Vec<u8> = vec![0u8; read_total as usize];

        // attempt to read from the buffer
        loop{
            match blk.read_blocks(media_id, array_lba, &mut buf) {
                Ok(a) => {
                    a.unwrap();
                    break;
                },
                Err(e) => match e {
                    _ => panic!("Found unexpected error: {:?}", e)
                }
            }
        }

        // now parse the data and add it to our partitions vector
        for i in 0..num_part as usize {
            partitions.push(
                GPTPartition::new(
                    buf[i*128 as usize..(i+1)*128 as usize]
                        .try_into().unwrap()
                )
            );
        }

        GPTDisk {
            media_id,
            blocksize: blk_size,
            header,
            partitions
        }
    }

    /// returns the number of partitions found in the GPT Table
    pub fn num_parts(&self) -> u32 {
        self.header.num_partitions()
    }

    /// returns the partitions
    pub fn partitions(&self) -> &Vec<GPTPartition> {
        &self.partitions
    }
}