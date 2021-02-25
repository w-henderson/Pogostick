#![no_std]
#![no_main]

mod vga;

use pog_os::interrupts::init_idt;
use vga::Writer;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("Hello, world!");

    pog_os::init_interrupt_handlers();
    x86_64::instructions::interrupts::int3();

    println!("Didn't crash pog");

    loop {}
}

#[panic_handler]
#[cfg(not(test))]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    println!("{}", _info);

    loop {}
}
