// Console output

use core::fmt::Write;
use lazy_static::lazy_static;
use spin::Mutex;
use volatile::Volatile;
use x86_64::instructions::{interrupts, port::Port};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
#[repr(u8)]
pub enum Colour {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct ColourCode(u8);

impl ColourCode {
    pub fn new(fg: Colour, bg: Colour) -> ColourCode {
        ColourCode((bg as u8) << 4 | (fg as u8))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct ScreenChar {
    ascii: u8,
    colour_code: ColourCode,
}

pub const BUFFER_HEIGHT: usize = 25;
pub const BUFFER_WIDTH: usize = 80;

#[repr(transparent)]
struct Buffer {
    chars: [[Volatile<ScreenChar>; BUFFER_WIDTH]; BUFFER_HEIGHT],
}

pub struct Writer {
    pub column_position: usize,
    colour_code: ColourCode,
    buffer: &'static mut Buffer,
}

impl Writer {
    pub fn new() -> Writer {
        Writer {
            column_position: 0,
            colour_code: ColourCode::new(Colour::White, Colour::Black),
            buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
        }
    }

    /// Set cursor position
    unsafe fn update_cursor(&mut self, x: usize, y: usize) {
        let mut cursor_port_1: Port<u8> = Port::new(0x3D4); // these two registers work together to store a `u16`
        let mut cursor_port_2: Port<u8> = Port::new(0x3D5); // they are separate though so we address them separately
        let pos = y as u16 * BUFFER_WIDTH as u16 + x as u16;

        // move the cursor to the given position
        cursor_port_1.write(0x0F);
        cursor_port_2.write((pos & 0xFF) as u8);
        cursor_port_1.write(0x0E);
        cursor_port_2.write(((pos >> 8) & 0xFF) as u8);
    }

    /// Write a character to the output
    pub fn write_char(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            byte => {
                if self.column_position >= BUFFER_WIDTH {
                    self.new_line();
                }

                let row = BUFFER_HEIGHT - 1;
                let col = self.column_position;

                self.buffer.chars[row][col].write(ScreenChar {
                    ascii: byte,
                    colour_code: self.colour_code,
                });

                self.column_position += 1;
                unsafe { self.update_cursor(self.column_position, BUFFER_HEIGHT - 1) };
            }
        }
    }

    /// Overwrite the last character of the output
    pub fn overwrite_char(&mut self, byte: u8) {
        self.column_position -= 1;
        self.write_char(byte);
        self.column_position -= 1;
        unsafe { self.update_cursor(self.column_position, BUFFER_HEIGHT - 1) };
    }

    /// Write a string to the output
    pub fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            match byte {
                0x20..=0x7e | b'\n' => self.write_char(byte), // printable
                _ => self.write_char(0xfe),                   // non printable
            }
        }
    }

    /// Write a coloured string to the output
    pub fn write_string_colour(&mut self, s: &str, colour: ColourCode) {
        self.colour_code = colour;
        self.write_string(s);
        self.colour_code = ColourCode::new(Colour::White, Colour::Black);
    }

    /// Write a character at a specific position to the output
    pub fn write_char_at(&mut self, byte: u8, row: usize, col: usize) {
        self.buffer.chars[row][col].write(ScreenChar {
            ascii: byte,
            colour_code: self.colour_code,
        });
    }

    /// Create a new line
    pub fn new_line(&mut self) {
        for row in 1..BUFFER_HEIGHT {
            for col in 0..BUFFER_WIDTH {
                let character = self.buffer.chars[row][col].read();
                self.buffer.chars[row - 1][col].write(character);
            }
        }
        self.clear_row(BUFFER_HEIGHT - 1);
        self.column_position = 0;
    }

    /// Clear a row of the output with blank characters
    fn clear_row(&mut self, row: usize) {
        let blank_char = ScreenChar {
            ascii: b' ',
            colour_code: ColourCode::new(Colour::White, Colour::Black),
        };
        for col in 0..BUFFER_WIDTH {
            self.buffer.chars[row][col].write(blank_char);
        }
    }
}

impl Write for Writer {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

lazy_static! {
    pub static ref WRITER: Mutex<Writer> = Mutex::new(Writer {
        column_position: 0,
        colour_code: ColourCode::new(Colour::White, Colour::Black),
        buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
    });
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::vga::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

pub fn err(string: &str) -> u8 {
    interrupts::without_interrupts(|| {
        let mut writer = WRITER.lock();
        writer.write_char(b'[');
        writer.write_string_colour(" ERR ", ColourCode::new(Colour::LightRed, Colour::Black));
        writer.write_string("]  ");
        writer.write_string(string);
    });
    1
}

pub fn warn(string: &str) {
    interrupts::without_interrupts(|| {
        let mut writer = WRITER.lock();
        writer.write_char(b'[');
        writer.write_string_colour(" WARN ", ColourCode::new(Colour::Yellow, Colour::Black));
        writer.write_string("] ");
        writer.write_string(string);
    });
}

pub fn info(string: &str) {
    interrupts::without_interrupts(|| {
        let mut writer = WRITER.lock();
        writer.write_char(b'[');
        writer.write_string_colour(" INFO ", ColourCode::new(Colour::LightCyan, Colour::Black));
        writer.write_string("] ");
        writer.write_string(string);
    });
}

pub fn okay(string: &str) -> u8 {
    interrupts::without_interrupts(|| {
        let mut writer = WRITER.lock();
        writer.write_char(b'[');
        writer.write_string_colour(" OKAY ", ColourCode::new(Colour::LightGreen, Colour::Black));
        writer.write_string("] ");
        writer.write_string(string);
    });
    0
}

#[doc(hidden)]
pub fn _print(args: core::fmt::Arguments) {
    interrupts::without_interrupts(|| {
        WRITER.lock().write_fmt(args).unwrap();
    });
}
