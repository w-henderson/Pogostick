// Handles keyboard interrupts
// Basically does everything to do with keyboard input

use crate::interrupts::{InterruptIndex, PICS};
use crate::print;
use crate::vga::WRITER;
use alloc::{string::String, vec::Vec};
use lazy_static::lazy_static;
use pc_keyboard::{layouts, DecodedKey, HandleControl, KeyCode, Keyboard, ScancodeSet1};
use spin::Mutex;
use x86_64::instructions::{interrupts, port::Port};
use x86_64::structures::idt::InterruptStackFrame;

pub struct Stdin {
    chars: Mutex<Vec<char>>,
    requesting: Mutex<bool>,
}

impl Stdin {
    /// Clear the standard input stream
    pub fn clear(&self) {
        let mut chars = self.chars.lock();
        *chars = Vec::new();
    }

    /// Get a character input (blocking)
    pub fn get_char(&self) -> char {
        let chars = self.chars.lock();
        let mut requesting = self.requesting.lock();
        let chars_len = chars.len();
        *requesting = true;
        drop(requesting);
        drop(chars);

        loop {
            let chars = self.chars.lock();
            let new_len = chars.len();

            drop(chars);

            if new_len != chars_len {
                break;
            }
            crate::idle();
        }

        let chars = self.chars.lock();
        let mut requesting = self.requesting.lock();
        *requesting = false;

        chars[chars.len() - 1]
    }

    /// Get a string input (blocking)
    pub fn get_str(&self) -> String {
        self.clear();
        let mut result = String::new();
        let mut new_char = self.get_char();

        while new_char != '\n' {
            if new_char == '\x08' {
                if let Some(_) = result.pop() {
                    interrupts::without_interrupts(|| {
                        let mut writer = WRITER.lock();
                        writer.overwrite_char(0x20);
                    });
                }
            } else {
                result.push(new_char);
            }
            new_char = self.get_char();
        }

        self.clear();

        result
    }
}

lazy_static! {
    pub static ref STDIN: Stdin = Stdin {
        chars: Mutex::new(Vec::new()),
        requesting: Mutex::new(false),
    };
}

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
    if *STDIN.requesting.lock() {
        let mut chars = STDIN.chars.lock();

        if character.is_alphanumeric() || character == '\n' || character == ' ' {
            chars.push(character);
            print!("{}", character);
        } else {
            // NON PRINTABLE CHARACTER HANDLING

            if character == '\x08' {
                // Handle backspace
                chars.push(character);
            }
        }
    }
}

fn handle_raw_key_input(_key: KeyCode) {
    /* TODO */
}
