#![no_std]
#![feature(abi_x86_interrupt)]

pub mod gdt;
pub mod interrupts;
pub mod vga;

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
pub fn idle() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}
