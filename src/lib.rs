#![no_std]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]

pub mod allocator; // heap allocation
pub mod ata; // drive management
pub mod conhost; // console input
pub mod gdt; // stack allocation for interrupts
pub mod input; // input handling
pub mod interrupts; // interrupt and exception handling
pub mod mem; // paging
pub mod time; // everything to do with time
pub mod vga; // console output
extern crate alloc; // lower level heap allocation

/// Initialises interrupt handling
pub fn init() {
    gdt::init();
    interrupts::init_idt();
    unsafe {
        interrupts::PICS.lock().initialize();
    }
    x86_64::instructions::interrupts::enable();
}

/// Forever sends halt instructions allowing the CPU to idle
pub fn idle_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}

/// Alias for `x86_64::instructions::hlt();`
pub fn idle() {
    x86_64::instructions::hlt();
}

#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    panic!("allocation error: {:?}", layout);
}
