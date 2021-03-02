use crate::ata::{self, Drive};
use crate::println;
use alloc::{borrow::ToOwned, string::String, vec::Vec};
use bit_field::BitField;
use lazy_static::lazy_static;
use spin::Mutex;

lazy_static! {
    pub static ref FILESYSTEM: Mutex<Option<FileSystem>> = Mutex::new(None);
}

/// Represents the filesystem
pub struct FileSystem {
    pub drive_index: u8,
    pub entry_sector: u32,
    pub entry_table: FileTableSector,
}

impl FileSystem {
    pub fn get_file(&self, path: &str) -> Option<File> {
        let mut current_sector = self.entry_sector;
        let mut current_table = self.entry_table.clone();

        while current_table
            .files
            .iter()
            .find(|el| el.name == path)
            .is_none()
        {
            if let Some(new_addr) = current_table.continuation_addr {
                current_sector = new_addr;
                current_table = FileTableSector::new(current_sector, self.drive_index as usize);
            } else {
                return None;
            }
        }

        return Some(
            current_table
                .files
                .iter()
                .find(|el| el.name == path)
                .unwrap()
                .clone(),
        );
    }

    pub fn write_file(&mut self, path: &str, bytes: Vec<u8>) {
        let mut current_sector = self.entry_sector;
        let mut current_table = &mut self.entry_table;
        let mut new_table: FileTableSector;

        while current_table.files.len() == 8 {
            if let Some(new_addr) = current_table.continuation_addr {
                current_sector = new_addr;
                new_table = FileTableSector::new(current_sector, self.drive_index as usize);
                current_table = &mut new_table;
            } else {
                let drives = ata::DRIVES.lock();
                let drive = &drives[self.drive_index as usize];
                let new_sector = drive.find_available_sector().unwrap();
                drop(drives);

                current_table.set_continuation(new_sector, self.drive_index as usize);
                new_table = FileTableSector::init(new_sector, self.drive_index as usize);
                current_table = &mut new_table;
            }
        }

        let drives = ata::DRIVES.lock();
        let new_file_sector = drives[self.drive_index as usize]
            .find_available_sector()
            .unwrap();

        drop(drives);

        current_table.add_file(path, new_file_sector);

        let drives = ata::DRIVES.lock();
        let drive = &drives[self.drive_index as usize];

        let mut bytes_to_write = bytes.clone();
        bytes_to_write.truncate(506);
        let mut written_bytes = bytes_to_write.len();
        let mut current_sector = DataSector::init(new_file_sector, drive, bytes_to_write);

        while written_bytes < bytes.len() {
            bytes_to_write = bytes.clone().drain(..written_bytes).collect();
            bytes_to_write.truncate(506);
            let extension_file_sector = drive.find_available_sector().unwrap();
            current_sector.continuation_addr = Some(extension_file_sector);
            current_sector.update_physical_drive(drive);
            written_bytes += bytes_to_write.len();
            current_sector = DataSector::init(extension_file_sector, drive, bytes_to_write);
        }
    }

    pub fn list_files(&self) -> Vec<String> {
        let mut result: Vec<String> = Vec::new();
        let start_table = &self.entry_table;
        result.extend(start_table.files.iter().map(|f| f.name.clone()));

        if start_table.continuation_addr.is_some() {
            let mut next_addr = start_table.continuation_addr.unwrap();
            loop {
                let table = FileTableSector::new(next_addr, self.drive_index as usize);
                result.extend(table.files.iter().map(|f| f.name.clone()));
                if table.continuation_addr.is_some() {
                    next_addr = table.continuation_addr.unwrap();
                } else {
                    break;
                }
            }
        };

        result
    }
}

#[derive(Clone, Debug)]
pub struct File {
    pub name: String,
    pub drive_index: usize,
    pub entry_addr: u32,
}

impl File {
    pub fn read(&self) -> Vec<u8> {
        let drives = ata::DRIVES.lock();
        let drive: &Drive = &drives[self.drive_index];

        let mut output_bytes: Vec<u8> = Vec::new();
        let mut current_addr = self.entry_addr;
        let mut current_sector = DataSector::new(current_addr, drive);

        loop {
            output_bytes.extend(
                current_sector.data[0..current_sector.size as usize]
                    .iter()
                    .cloned(),
            );
            if let Some(next_sector) = current_sector.continuation_addr {
                current_addr = next_sector;
                current_sector = DataSector::new(current_addr, drive);
            } else {
                break;
            }
        }

        output_bytes
    }
}

/// Represents a sector of the disk containing a file table
#[derive(Clone)]
pub struct FileTableSector {
    pub addr: u32,
    pub continuation_addr: Option<u32>,
    pub files: Vec<File>,
    pub drive_index: usize,
}

impl FileTableSector {
    //// Create a new `FileTableSector` object from its address
    pub fn new(addr: u32, drive_index: usize) -> Self {
        let drive: &Drive = &ata::DRIVES.lock()[drive_index];

        let mut buf = [0_u8; 512];
        drive.read(addr, &mut buf);

        drop(drive);

        // Parse the continuation address from the first four bytes
        let continuation_addr =
            (buf[0] as u32) << 24 | (buf[1] as u32) << 16 | (buf[2] as u32) << 8 | (buf[3] as u32);
        let continuation_option = if continuation_addr != 0 {
            Some(continuation_addr)
        } else {
            None
        };

        // Parse the actual filenames and file addresses information
        let mut files: Vec<File> = Vec::new();

        let data_bytes = &buf[4..508]; // bytes 508 - 511 are ignored as they contain "POGO"
        for i in 0_usize..8 {
            let file_bytes = &data_bytes[i * 63..(i + 1) * 63];
            let file_name_bytes = &file_bytes[0..59];
            let file_addr_bytes = &file_bytes[59..63];
            let file_addr = (file_addr_bytes[0] as u32) << 24
                | (file_addr_bytes[1] as u32) << 16
                | (file_addr_bytes[2] as u32) << 8
                | (file_addr_bytes[3] as u32);

            if file_addr != 0 {
                let mut file_name = String::new();
                for byte in file_name_bytes {
                    if *byte != 0 {
                        file_name.push(char::from_u32(*byte as u32).unwrap());
                    } else {
                        break;
                    }
                }
                files.push(File {
                    name: file_name,
                    entry_addr: file_addr,
                    drive_index,
                });
            }
        }

        FileTableSector {
            addr,
            continuation_addr: continuation_option,
            files,
            drive_index,
        }
    }

    /// Initialise a brand new sector on the disk, then return a virtual instance of it
    pub fn init(new_addr: u32, drive_index: usize) -> Self {
        let drive: &Drive = &ata::DRIVES.lock()[drive_index];

        let mut init_buf = [0_u8; 512];
        init_buf[508] = b'P';
        init_buf[509] = b'O';
        init_buf[510] = b'G';
        init_buf[511] = b'O';

        drive.write(new_addr, &init_buf);

        FileTableSector {
            addr: new_addr,
            continuation_addr: None,
            files: Vec::new(),
            drive_index,
        }
    }

    /// Update the virtual parameters onto the disk
    pub fn update_physical_drive(&self) {
        let drive: &Drive = &ata::DRIVES.lock()[self.drive_index];
        let mut buf = [0_u8; 512];

        if let Some(continuation) = self.continuation_addr {
            buf[0] = continuation.get_bits(0..8) as u8;
            buf[1] = continuation.get_bits(8..16) as u8;
            buf[2] = continuation.get_bits(16..24) as u8;
            buf[3] = continuation.get_bits(24..32) as u8;
        } else {
            buf[0] = 0;
            buf[1] = 0;
            buf[2] = 0;
            buf[3] = 0;
        }

        let mut index = 4;
        for file in &self.files {
            for (current_index, byte) in file.name.bytes().enumerate() {
                buf[index + current_index] = byte;
            }

            buf[index + 59] = file.entry_addr.get_bits(0..8) as u8;
            buf[index + 60] = file.entry_addr.get_bits(8..16) as u8;
            buf[index + 61] = file.entry_addr.get_bits(16..24) as u8;
            buf[index + 62] = file.entry_addr.get_bits(24..32) as u8;

            index += 63;
        }

        buf[508] = b'P';
        buf[509] = b'O';
        buf[510] = b'G';
        buf[511] = b'O';

        drive.write(self.addr, &buf);
    }

    /// Set the continuation address on disk
    pub fn set_continuation(&mut self, sector: u32, drive_index: usize) {
        self.continuation_addr = Some(sector);
        self.update_physical_drive();
    }

    pub fn add_file(&mut self, path: &str, addr: u32) {
        assert!(self.files.len() < 8);
        self.files.push(File {
            name: path.to_owned(),
            drive_index: self.drive_index,
            entry_addr: addr,
        });
        self.update_physical_drive();
    }
}

/// Represents a sector of the disk containing data
pub struct DataSector {
    pub addr: u32,
    pub continuation_addr: Option<u32>,
    pub size: u16,
    pub data: [u8; 506],
    pub drive_index: usize,
}

impl DataSector {
    /// Creates a new `DataSector` object from its address
    pub fn new(addr: u32, drive: &Drive) -> Self {
        let mut buf = [0_u8; 512];
        drive.read(addr, &mut buf);

        let continuation_addr =
            (buf[0] as u32) << 24 | (buf[1] as u32) << 16 | (buf[2] as u32) << 8 | (buf[3] as u32);

        let continuation_addr_option = if continuation_addr != 0 {
            Some(continuation_addr)
        } else {
            None
        };

        let size = (buf[4] as u16) << 8 | (buf[5] as u16);
        let mut data = [0_u8; 506];
        data.clone_from_slice(&buf[6..512]);

        DataSector {
            addr,
            continuation_addr: continuation_addr_option,
            size,
            data,
            drive_index: drive.drive_index as usize,
        }
    }

    /// Initialise a brand new `DataSector` object on disk, then return a virtual instance
    pub fn init(addr: u32, drive: &Drive, bytes: Vec<u8>) -> Self {
        let mut buf = [0_u8; 512];
        let size = bytes.len() as u16;
        buf[4] = size.get_bits(0..8) as u8;
        buf[5] = size.get_bits(8..16) as u8;

        let index = 6;
        for (current_index, byte) in bytes.iter().enumerate() {
            buf[index + current_index] = *byte;
        }

        drive.write(addr, &buf);
        return Self::new(addr, drive);
    }

    pub fn update_physical_drive(&self, drive: &Drive) {
        let mut buf = [0_u8; 512];
        drive.read(self.addr, &mut buf);

        if let Some(continuation) = self.continuation_addr {
            buf[0] = continuation.get_bits(0..8) as u8;
            buf[1] = continuation.get_bits(8..16) as u8;
            buf[2] = continuation.get_bits(16..24) as u8;
            buf[3] = continuation.get_bits(24..32) as u8;
        } else {
            buf[0] = 0;
            buf[1] = 0;
            buf[2] = 0;
            buf[3] = 0;
        }

        for index in 6_usize..512 {
            buf[index] = self.data[index - 6];
        }

        drive.write(self.addr, &buf);
    }
}

/// Create the basic filesystem
fn create_fs(drive_index: u8) {
    let drives = ata::DRIVES.lock();
    let mut filesystem = FILESYSTEM.lock();

    let mut init_buf = [0_u8; 512];
    init_buf[508] = b'P';
    init_buf[509] = b'O';
    init_buf[510] = b'G';
    init_buf[511] = b'O';

    let drive = &drives[drive_index as usize];
    drive.write(drive.sectors - 1, &init_buf);

    let sectors = drive.sectors;
    drop(drives);

    *filesystem = Some(FileSystem {
        drive_index: drive_index,
        entry_sector: sectors - 1,
        entry_table: FileTableSector::new(sectors - 1, drive_index as usize),
    });
}

/// Try to detect a filesystem on any drive.
/// Gives the option to create one if none is found.
pub fn detect_fs() {
    {
        let drives = ata::DRIVES.lock();
        let mut filesystem = FILESYSTEM.lock();
        let filesystem_signature: [u8; 4] = [b'P', b'O', b'G', b'O'];

        for drive in &*drives {
            let mut buf = [0_u8; 512];
            let entry_sector = drive.sectors - 1;
            drive.read(entry_sector, &mut buf);
            if &buf[508..512] == &filesystem_signature {
                let drive_index = drive.drive_index;
                drop(drives);
                *filesystem = Some(FileSystem {
                    drive_index: drive_index,
                    entry_sector,
                    entry_table: FileTableSector::new(entry_sector, drive_index as usize),
                });
                break;
            }
        }
    }

    let filesystem = FILESYSTEM.lock();

    if let Some(fs) = &*filesystem {
        println!(
            "filesystem detected on drive {}, entry sector at {}",
            fs.drive_index, fs.entry_sector
        );
        let files_len = fs.entry_table.files.len();
        println!("detected files: {}", files_len);
        println!("first file name: {}", fs.entry_table.files[0].name);
        println!("first file addr: {}", fs.entry_table.files[0].entry_addr);
    } else {
        println!("no filesystem detected, initialising one");
        drop(filesystem);

        create_fs(0);
    }
}

pub fn is_mounted() -> bool {
    FILESYSTEM.lock().is_some()
}
