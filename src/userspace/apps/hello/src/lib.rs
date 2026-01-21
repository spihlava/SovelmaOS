#![no_std]

use sovelma_sdk::{get_root, print_str, yield_now};

#[no_mangle]
pub extern "C" fn _start() {
    print_str("Hello from WASM with Capabilities!\n");

    let _root_cap = get_root();
    print_str("Root capability acquired.\n");

    yield_now();
    print_str("Yielded and resumed!\n");
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
