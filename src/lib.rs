#![no_std]
#![feature(abi_x86_interrupt)]

pub mod interrupts;
pub mod vga;

pub fn init_interrupt_handlers() {
    interrupts::init_idt();
}
