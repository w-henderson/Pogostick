use crate::time::wait_nano;
use alloc::{string::String, vec::Vec};
use bit_field::BitField;
use core::hint::spin_loop;
use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::instructions::port::{Port, PortReadOnly, PortWriteOnly};

/// Represents a command to send to the drive.
#[repr(u16)]
enum DriveCommand {
    Read = 0x20,
    Write = 0x30,
    Identify = 0xEC,
}

/// Represents the status of the drive.
/// Not all variants are implemented, but they're here for future improvements.
#[allow(dead_code)]
#[repr(usize)]
enum DriveStatus {
    Error = 0,          // ERR
    Index = 1,          // IDX
    Corrected = 2,      // CORR
    Queued = 3,         // DRQ
    ServiceRequest = 4, // SRV
    DriveFault = 5,     // DF
    Ready = 6,          // RDY
    Busy = 7,           // BSY
}

/// Represents a bus.
/// Currently only works with the secondary bus.
/// TODO: fix
#[derive(Debug, Clone)]
pub struct Bus {
    id: u8,
    irq: u8,

    data_reg: Port<u16>,
    error_reg: PortReadOnly<u8>,
    features_reg: PortWriteOnly<u8>,
    sector_count_reg: Port<u8>,
    lba0_reg: Port<u8>,
    lba1_reg: Port<u8>,
    lba2_reg: Port<u8>,
    drive_reg: Port<u8>,
    status_reg: PortReadOnly<u8>,
    command_reg: PortWriteOnly<u8>,

    alt_status_reg: PortReadOnly<u8>,
    control_reg: PortWriteOnly<u8>,
    drive_blockess_reg: PortReadOnly<u8>,
}

impl Bus {
    /// Initialises a new bus by creating port references.
    pub fn new(id: u8, io_base: u16, control_base: u16, irq: u8) -> Self {
        Self {
            id,
            irq,

            data_reg: Port::new(io_base + 0),
            error_reg: PortReadOnly::new(io_base + 1),
            features_reg: PortWriteOnly::new(io_base + 1),
            sector_count_reg: Port::new(io_base + 2),
            lba0_reg: Port::new(io_base + 3),
            lba1_reg: Port::new(io_base + 4),
            lba2_reg: Port::new(io_base + 5),
            drive_reg: Port::new(io_base + 6),
            command_reg: PortWriteOnly::new(io_base + 7),
            status_reg: PortReadOnly::new(io_base + 7),

            alt_status_reg: PortReadOnly::new(control_base + 0),
            control_reg: PortWriteOnly::new(control_base + 1),
            drive_blockess_reg: PortReadOnly::new(control_base + 1),
        }
    }

    /// Sends a reset command to the drive.
    unsafe fn reset(&mut self) {
        self.control_reg.write(4);
        wait_nano(5);
        self.control_reg.write(0);
        wait_nano(2000);
    }

    /// Waits for a command to be processed.
    /// This is done by reading the status 16 times, as advised on the OSDev wiki.
    unsafe fn wait(&mut self) {
        for _ in 0..4 {
            self.alt_status_reg.read();
        }
    }

    /// Waits until the bus is no longer busy.
    unsafe fn busy_loop(&mut self) {
        self.wait();
        while self.is_busy() {
            spin_loop();
        }
    }

    /// Detects if the bus is currently busy.
    unsafe fn is_busy(&mut self) -> bool {
        self.status_reg.read().get_bit(DriveStatus::Busy as usize)
    }

    /// Detects if the bus has errorred.
    unsafe fn is_error(&mut self) -> bool {
        self.status_reg.read().get_bit(DriveStatus::Error as usize)
    }

    /// Detects if the bus is ready for a command.
    unsafe fn is_ready(&mut self) -> bool {
        self.status_reg.read().get_bit(DriveStatus::Ready as usize)
    }

    /// Selects the specified drive on the bus.
    unsafe fn select_drive(&mut self, drive: u8) {
        let drive_id = 0xA0 | (drive << 4);
        self.drive_reg.write(drive_id);
    }

    /// Sets up the given drive to read or write to a certain block.
    unsafe fn setup(&mut self, drive: u8, block: u32) {
        let drive_id = 0xE0 | (drive << 4);
        self.drive_reg
            .write(drive_id | ((block.get_bits(24..28) as u8) & 0x0F));
        self.sector_count_reg.write(1);
        self.lba0_reg.write(block.get_bits(0..8) as u8);
        self.lba1_reg.write(block.get_bits(8..16) as u8);
        self.lba2_reg.write(block.get_bits(16..24) as u8);
    }

    /// Sends an IDENTIFY command to the drive.
    /// Returns `Some([u16; 256])` if the drive successfully identified itself.
    /// Returns `None` if the drive did not identify itself.
    pub unsafe fn identify_drive(&mut self, drive: u8) -> Option<[u16; 256]> {
        self.reset();
        self.wait();
        self.select_drive(drive);
        self.sector_count_reg.write(0);
        self.lba0_reg.write(0);
        self.lba1_reg.write(0);
        self.lba2_reg.write(0);
        self.command_reg.write(DriveCommand::Identify as u8);

        if self.status_reg.read() == 0 {
            return None;
        }

        self.busy_loop();

        let read1 = self.lba1_reg.read();
        let read2 = self.lba2_reg.read();

        if read1 != 0 || read2 != 0 {
            return None;
        }

        for i in 0.. {
            if i == 256 {
                self.reset();
                return None;
            }
            if self.is_error() {
                return None;
            }
            if self.is_ready() {
                break;
            }
        }

        let mut res = [0; 256];
        for i in 0..256 {
            res[i] = self.data_reg.read();
        }
        Some(res)
    }

    /// Reads from the given block into the specified buffer.
    pub unsafe fn read(&mut self, drive: u8, block: u32, buf: &mut [u8]) {
        self.setup(drive, block);
        self.command_reg.write(DriveCommand::Read as u8);
        self.busy_loop();

        for i in 0..256 {
            let data = self.data_reg.read();
            buf[i * 2] = data.get_bits(0..8) as u8;
            buf[i * 2 + 1] = data.get_bits(8..16) as u8;
        }
    }

    /// Writes to the given block from the specified buffer.
    pub unsafe fn write(&mut self, drive: u8, block: u32, buf: &[u8]) {
        self.setup(drive, block);
        self.command_reg.write(DriveCommand::Write as u8);
        self.busy_loop();

        for i in 0..256 {
            let mut data = 0 as u16;
            data.set_bits(0..8, buf[i * 2] as u16);
            data.set_bits(8..16, buf[i * 2 + 1] as u16);
            self.data_reg.write(data);
        }

        self.busy_loop();
    }
}

lazy_static! {
    pub static ref BUSES: Mutex<Vec<Bus>> = Mutex::new(Vec::new());
    pub static ref DRIVES: Mutex<Vec<Drive>> = Mutex::new(Vec::new());
}

/// Represents a generic ATA drive
pub struct Drive {
    pub bus_index: u8,
    pub drive_index: u8,
    pub model: String,
    pub serial: String,
    pub sectors: u32,
}

impl Drive {
    /// Reads 512 bytes from the disk at the specified block.
    /// Writes these bytes to the given buffer.
    pub fn read(&self, block: u32, mut buf: &mut [u8]) {
        let mut buses = BUSES.lock();
        unsafe { buses[self.bus_index as usize].read(self.drive_index, block, &mut buf) };
    }

    /// Writes a buffer of 512 bytes to the disk at the specified block.
    /// Buffer must be 512 bytes.
    pub fn write(&self, block: u32, buf: &[u8]) {
        let mut buses = BUSES.lock();
        unsafe { buses[self.bus_index as usize].write(self.drive_index, block, &buf) };
    }

    /// Finds an available sector on the disk.
    /// If none is found (e.g. the disk is full), returns None.
    pub fn find_available_sector(&self) -> Option<u32> {
        let mut current_sector = self.sectors - 1;

        while current_sector > 0 {
            let mut buf = [0_u8; 512];
            self.read(current_sector, &mut buf);
            if buf.iter().all(|el| *el == 0) {
                return Some(current_sector);
            }
            current_sector -= 1;
        }

        None
    }
}

/// Initialise and identify ATA drives
pub fn init() {
    let mut buses = BUSES.lock();
    let mut drives = DRIVES.lock();

    //buses.push(Bus::new(0, 0x1F0, 0x3F6, 14)); doesn't work for some reason
    buses.push(Bus::new(1, 0x170, 0x376, 15));

    for drive in 0..2 {
        if let Some(buf) = unsafe { buses[0_usize].identify_drive(drive) } {
            let mut serial = String::new();
            for i in 10..20 {
                for &b in &buf[i].to_be_bytes() {
                    serial.push(b as char);
                }
            }
            serial = serial.trim().into();

            let mut model = String::new();
            for i in 27..47 {
                for &b in &buf[i].to_be_bytes() {
                    model.push(b as char);
                }
            }
            model = model.trim().into();

            let sectors = (buf[61] as u32) << 16 | (buf[60] as u32);

            drives.push(Drive {
                bus_index: 0,
                drive_index: drive,
                model,
                serial,
                sectors,
            });
        }
    }
}
