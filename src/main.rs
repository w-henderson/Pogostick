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

    let mut fs = pogostick::fs::FILESYSTEM.lock();
    let filesystem = fs.as_mut().unwrap();
    println!("locked fs");
    println!("{:?}", filesystem.get_file("pogchamp"));
    filesystem.write_file("pogchamp", vec![1, 2, 3, 4]);
    println!("{:?}", filesystem.get_file("pogchamp"));

    conhost::console_loop();
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    println!("{}", _info);
    pogostick::idle_loop();
}
