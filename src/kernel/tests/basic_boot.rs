#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(sovelma_kernel::testutil::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;
use sovelma_kernel::testutil::{exit_qemu, QemuExitCode};

#[no_mangle]
pub extern "C" fn _start() -> ! {
    test_main();
    exit_qemu(QemuExitCode::Success);
    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    sovelma_kernel::testutil::test_panic_handler(info)
}

#[test_case]
fn trivial_assertion() {
    assert_eq!(1, 1);
}
