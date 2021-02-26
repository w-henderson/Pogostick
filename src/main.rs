#![no_std]
#![no_main]

mod vga;

use pog_os::input::stdin;
use pog_os::interrupts::init_idt;
use vga::Writer;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    pog_os::init();

    println!("Hello, world!");

    let new_char = stdin.get_char();

    println!("Char: {}", new_char);

    pog_os::idle_loop();
}

#[panic_handler]
#[cfg(not(test))]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    println!("{}", _info);

    pog_os::idle_loop();
}
