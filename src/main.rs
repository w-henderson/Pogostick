#![no_std]
#![no_main]

mod vga;

use vga::Writer;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("Hello, world!");

    loop {}
}

#[panic_handler]
#[cfg(not(test))]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    println!("{}", _info);

    loop {}
}
