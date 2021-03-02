#![no_std]
#![no_main]

extern crate alloc;
use alloc::vec;
use bootloader::{entry_point, BootInfo};
use pogostick::{conhost, println};

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    pogostick::init(boot_info);
    pogostick::fs::detect_fs();

    {
        let mut fs = pogostick::fs::FILESYSTEM.lock();
        let filesystem = fs.as_mut().unwrap();
        filesystem.write_file("testbigfile", [0xff_u8; 1000].to_vec());
    }

    conhost::console_loop();
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    println!("{}", _info);
    pogostick::idle_loop();
}
