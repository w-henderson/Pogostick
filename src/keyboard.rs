// Handles keyboard interrupts
// Basically does everything to do with the keyboard

use crate::interrupts::{InterruptIndex, PICS};
use crate::{print, println};
use lazy_static::lazy_static;
use pc_keyboard::{layouts, DecodedKey, HandleControl, KeyCode, Keyboard, ScancodeSet1};
use spin::Mutex;
use x86_64::instructions::port::Port;
use x86_64::structures::idt::InterruptStackFrame;

/// Keyboard interrupt handler, manages keyboard input
pub extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: &mut InterruptStackFrame) {
    lazy_static! {
        static ref KEYBOARD: Mutex<Keyboard<layouts::Uk105Key, ScancodeSet1>> = Mutex::new(
            Keyboard::new(layouts::Uk105Key, ScancodeSet1, HandleControl::Ignore)
        );
    }

    let mut keyboard = KEYBOARD.lock();
    let mut port = Port::new(0x60); // keyboard data port
    let scancode: u8 = unsafe { port.read() }; // get scancode

    if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
        if let Some(key) = keyboard.process_keyevent(key_event) {
            match key {
                DecodedKey::Unicode(new_character) => handle_raw_char_input(new_character),
                DecodedKey::RawKey(key) => handle_raw_key_input(key),
            }
        }
    }

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}

fn handle_raw_char_input(character: char) {
    print!("{}", character);
}

fn handle_raw_key_input(key: KeyCode) {
    print!("{:?}", key);
}
