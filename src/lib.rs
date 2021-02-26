#![no_std]
#![feature(abi_x86_interrupt)]

pub mod gdt; // stack allocation for interrupts
pub mod input; // input handling
pub mod interrupts; // interrupt and exception handling
pub mod mem; // heap allocation
pub mod vga; // console output

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
