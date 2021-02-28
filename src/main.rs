#![no_std]
#![no_main]

extern crate alloc;
use bootloader::{entry_point, BootInfo};
use pog_os::{allocator, conhost, mem, println};
use x86_64::VirtAddr;

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    // === BEGIN INIT SECTION ===
    pog_os::init();
    let physical_memory_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { mem::init(physical_memory_offset) };
    let mut frame_allocator = unsafe { mem::BootInfoFrameAllocator::new(&boot_info.memory_map) };
    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap init failed");
    // === END INIT SECTION ===

    conhost::console_loop();
}

#[panic_handler]
#[cfg(not(test))]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    println!("{}", _info);

    pog_os::idle_loop();
}
