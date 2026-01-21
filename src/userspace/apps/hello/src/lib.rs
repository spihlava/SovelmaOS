#![no_std]

use sovelma_sdk::{print, get_temp};

#[no_mangle]
pub extern "C" fn start() {
    print();
    let _temp = get_temp();
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
