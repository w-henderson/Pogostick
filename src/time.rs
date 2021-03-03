use alloc::string::{String, ToString};
use alloc::{format, vec::Vec};
use core::hint::spin_loop;
use x86_64::instructions::port::Port;

/// Represents a time
#[allow(dead_code)]
pub struct Time {
    second: u8,
    minute: u8,
    hour: u8,
    weekday: u8,
    day: u8,
    month: u8,
    year: u8,
}

impl Time {
    /// Get the current time
    pub fn get() -> Self {
        let mut control_port: Port<u8> = Port::new(0x70);
        let mut data_port: Port<u8> = Port::new(0x71);

        let mut raw_values: Vec<u8> = Vec::new();

        // Iterate over registers and read them
        for register in &[0x00_u8, 0x02, 0x04, 0x06, 0x07, 0x08, 0x09, 0x0B] {
            unsafe {
                control_port.write(*register);
                raw_values.push(data_port.read());
            }
        }

        // BCD Mode (https://wiki.osdev.org/CMOS#Format_of_Bytes)
        // Basically means you have to do this weird algorithm to get the right number
        if raw_values[7] & 0x04 == 0 {
            raw_values[0] = (raw_values[0] & 0x0F) + ((raw_values[0] / 16) * 10);
            raw_values[1] = (raw_values[1] & 0x0F) + ((raw_values[1] / 16) * 10);
            raw_values[2] = ((raw_values[2] & 0x0F) + (((raw_values[2] & 0x70) / 16) * 10))
                | (raw_values[2] & 0x80);
            raw_values[4] = (raw_values[4] & 0x0F) + ((raw_values[4] / 16) * 10);
            raw_values[5] = (raw_values[5] & 0x0F) + ((raw_values[5] / 16) * 10);
            raw_values[6] = (raw_values[6] & 0x0F) + ((raw_values[6] / 16) * 10);
        }

        // 12-hour to 24-hour conversion
        if (raw_values[7] & 0x02 == 0) && (raw_values[2] & 0x80 == 0) {
            raw_values[2] = ((raw_values[2] & 0x7F) + 12) % 24;
        }

        Time {
            second: raw_values[0],
            minute: raw_values[1],
            hour: raw_values[2],
            weekday: raw_values[3],
            day: raw_values[4],
            month: raw_values[5],
            year: raw_values[6],
        }
    }

    /// Get the name of the day, e.g. Monday
    pub fn get_day_name(&self) -> &'static str {
        match self.weekday {
            1 => "Sunday",
            2 => "Monday",
            3 => "Tuesday",
            4 => "Wednesday",
            5 => "Thursday",
            6 => "Friday",
            7 => "Saturday",
            _ => "Error",
        }
    }

    /// Get the name of the current month, e.g. January
    pub fn get_month_name(&self) -> &'static str {
        match self.month {
            1 => "January",
            2 => "February",
            3 => "March",
            4 => "April",
            5 => "May",
            6 => "June",
            7 => "July",
            8 => "August",
            9 => "September",
            10 => "October",
            11 => "November",
            12 => "December",
            _ => "Error",
        }
    }
}

impl ToString for Time {
    fn to_string(&self) -> String {
        format!(
            "{:02}:{:02}, {} {} {} 20{:02}",
            self.hour,
            self.minute,
            self.get_day_name(),
            self.day,
            self.get_month_name(),
            self.year
        )
    }
}

pub fn rdtsc() -> u64 {
    unsafe {
        core::arch::x86_64::_mm_lfence();
        core::arch::x86_64::_rdtsc()
    }
}

pub fn wait_nano(nanoseconds: u64) {
    let start = rdtsc();
    while rdtsc() - start < nanoseconds * 4 {
        spin_loop();
    }
}
