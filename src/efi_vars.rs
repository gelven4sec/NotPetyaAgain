use alloc::vec;
use core::str;

use uefi::table::runtime::{VariableAttributes, VariableVendor};
use uefi::table::{Boot, SystemTable};
use uefi::{CStr16, Guid};

const VENDOR: &VariableVendor = &VariableVendor(Guid::from_values(69, 69, 69, 69, 69));

pub fn write_var(st: &SystemTable<Boot>, var_name: &str, buf: &[u8]) -> uefi::Result {
    let mut var_name_buf = vec![0u16; var_name.len() + 1];
    let var_name = CStr16::from_str_with_buf(var_name, &mut var_name_buf).unwrap();

    let attributes = VariableAttributes::BOOTSERVICE_ACCESS
        | VariableAttributes::RUNTIME_ACCESS
        | VariableAttributes::NON_VOLATILE;

    st.runtime_services()
        .set_variable(var_name, VENDOR, attributes, buf)?;

    Ok(())
}

pub fn read_var<'a>(
    st: &SystemTable<Boot>,
    var_name: &str,
    buf: &'a mut [u8],
) -> uefi::Result<&'a [u8]> {
    let mut var_name_buf = vec![0u16; var_name.len() + 1];
    let var_name = CStr16::from_str_with_buf(var_name, &mut var_name_buf).unwrap();

    let (data, _) = st.runtime_services().get_variable(var_name, VENDOR, buf)?;

    Ok(data)
}
