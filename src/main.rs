#![no_std]
#![no_main]

extern crate alloc;
use bootloader::{entry_point, BootInfo};
use pogostick::{conhost, println};

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    pogostick::init(boot_info);
    conhost::console_loop();
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    println!("{}", _info);
    pogostick::idle_loop();
}
