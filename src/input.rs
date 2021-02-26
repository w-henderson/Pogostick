// Handles keyboard interrupts
// Basically does everything to do with keyboard input

use crate::interrupts::{InterruptIndex, PICS};
use crate::{print, println};
use lazy_static::lazy_static;
use pc_keyboard::{layouts, DecodedKey, HandleControl, KeyCode, Keyboard, ScancodeSet1};
use spin::Mutex;
use x86_64::instructions::port::Port;
use x86_64::structures::idt::InterruptStackFrame;

pub struct Stdin {
    chars: Mutex<[char; 64]>,
    location: Mutex<usize>,
}

impl Stdin {
    pub fn clear(&self) {
        let mut chars = self.chars.lock();
        let mut loc = self.location.lock();
        *chars = ['\0'; 64];
        *loc = 0_usize;
    }

    pub fn get_char(&self) -> char {
        let loc = self.location.lock();
        let last_loc = *loc;
        drop(loc);

        loop {
            let loc = self.location.lock();
            let new_loc = *loc;

            drop(loc);

            if new_loc != last_loc {
                break;
            }
            crate::idle();
        }

        let chars = self.chars.lock();

        chars[last_loc]
    }

    /* TODO
    pub fn get_str(&self) -> [char; 64] {
        let mut new_char_arr: [char; 64] = ['\0'; 64];
        let mut new_char = '\0';
        let mut index: usize = 0;

        while index < 64 && new_char != '\n' {
            new_char = self.get_char();
            new_char_arr[index] = new_char;
            index += 1;
        }

        new_char_arr
    }*/
}

lazy_static! {
    pub static ref STDIN: Stdin = Stdin {
        chars: Mutex::new(['\0'; 64]),
        location: Mutex::new(0_usize)
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
    let mut chars = STDIN.chars.lock();
    let mut loc = STDIN.location.lock();
    chars[*loc] = character;
    *loc += 1;
}

fn handle_raw_key_input(key: KeyCode) {
    print!("{:?}", key);
}
