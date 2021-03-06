use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering};
use core::{fmt::Display, hint::spin_loop};
use x86_64::instructions::interrupts::without_interrupts;
use x86_64::instructions::port::Port;

static TICKS: AtomicUsize = AtomicUsize::new(0); // ticks since start
const PIT_DIVIDER: usize = 1193; // divider for PIT frequency (see OSDev wiki)
const PIT_INTERVAL: f64 = PIT_DIVIDER as f64 / (3_579_545.0 / 3.0); // interval between PIT ticks

pub fn init() {
    without_interrupts(|| {
        let divider_bytes = (PIT_DIVIDER as u16).to_le_bytes();
        let mut control_port: Port<u8> = Port::new(0x43);
        let mut data_port: Port<u8> = Port::new(0x40);
        unsafe {
            //  00 - channel 0, generates interrupts
            //  11 - access mode lobyte/hibyte
            // 011 - square wave generator
            //   0 - binary mode
            control_port.write(0b0011_0110);
            data_port.write(divider_bytes[0]);
            data_port.write(divider_bytes[1]);
        }
    });
}

/// Get the current system uptime in seconds.
/// Not necessarily accurate over larger periods of time.
/// Generally accurate +/- 5% over n seconds.
/// TODO: make more accurate
pub fn uptime() -> f64 {
    PIT_INTERVAL * TICKS.load(Ordering::Relaxed) as f64
}

pub fn handle_pit_interrupt() {
    // For some reason it's exactly half the correct speed so add 2 instead of 1
    // TODO: figure out why
    TICKS.fetch_add(2, Ordering::Relaxed);
}

/// Represents a time
#[allow(dead_code)]
pub struct DateTime {
    second: u8,
    minute: u8,
    hour: u8,
    weekday: u8,
    day: u8,
    month: u8,
    year: u8,
}

impl DateTime {
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

        DateTime {
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

impl Display for DateTime {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
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

/// Gets number of CPU operations completed
pub fn rdtsc() -> u64 {
    unsafe {
        core::arch::x86_64::_mm_lfence();
        core::arch::x86_64::_rdtsc()
    }
}

/// Waits for the specified number of nanoseconds.
/// HIGHLY INACCURATE, DON'T USE!
/// TODO: FIX
pub fn wait_nano(nanoseconds: u64) {
    let start = rdtsc();
    while rdtsc() - start < nanoseconds {
        spin_loop();
    }
}
