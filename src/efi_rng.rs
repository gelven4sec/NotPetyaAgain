use core::num::NonZeroU32;
use rand_core::{CryptoRng, Error, RngCore};
use uefi::proto::rng::Rng;
use uefi::table::{Boot, SystemTable};

pub struct EfiRng (&'static mut Rng);

impl EfiRng {
    pub fn new(st: &SystemTable<Boot>) -> uefi::Result<Self> {
        let rng = st.boot_services().locate_protocol::<Rng>()?;
        let rng = unsafe { &mut *rng.get() };

        Ok(Self(rng))
    }
}

impl CryptoRng for EfiRng {}

impl RngCore for EfiRng {
    fn next_u32(&mut self) -> u32 {
        self.next_u64() as u32
    }

    fn next_u64(&mut self) -> u64 {
        let mut buf = [0u8; 8];
        self.0.get_rng(None, &mut buf).unwrap();

        u64::from_ne_bytes(buf)
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        self.0.get_rng(None, dest).unwrap();
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Error> {
        if self.0.get_rng(None, dest).is_err() {
            Err(Error::from(NonZeroU32::new(1).unwrap() ))
        }
        else { Ok(()) }
    }
}