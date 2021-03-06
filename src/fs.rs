use crate::ata::{self, Drive};
use crate::input::STDIN;
use crate::vga::{info, okay, warn};
use crate::{println, ExitCode};
use alloc::{borrow::ToOwned, format, string::String, vec::Vec};
use bit_field::BitField;
use lazy_static::lazy_static;
use spin::Mutex;

lazy_static! {
    pub static ref FILESYSTEM: Mutex<Option<FileSystem>> = Mutex::new(None);
}

/// Struct representing the filesystem.
pub struct FileSystem {
    pub drive_index: u8,
    pub entry_sector: u32,
    pub entry_table: FileTableSector,
}

impl FileSystem {
    /// Get a file at the given path from the filesystem, or None if not found
    pub fn get_file(&self, path: &Vec<String>) -> Option<File> {
        if let Some(table) = self.get_table_with_object(path) {
            table.get_file(&path[path.len() - 1])
        } else {
            None
        }
    }

    /// Get a directory at the given path from the filesystem, or None if not found
    pub fn get_dir(&self, path: &Vec<String>) -> Option<Dir> {
        if let Some(table) = self.get_table_with_object(path) {
            table.get_dir(&path[path.len() - 1])
        } else {
            None
        }
    }

    /// Gets a file table sector containing the given file or directory.
    fn get_table_with_object(&self, path: &Vec<String>) -> Option<FileTableSector> {
        let mut current_table = self.entry_table.clone();
        let mut current_dir: Option<String> = None;

        // Iterate over the objects of the path
        for (index, obj) in path.iter().enumerate() {
            // Iterate over the tables representing the dir
            while !current_table.contains_object(obj) {
                if let Some(new_addr) = current_table.continuation_addr {
                    current_table = FileTableSector::load(
                        new_addr,
                        self.drive_index as usize,
                        current_dir.clone(),
                    );
                } else {
                    return None;
                }
            }

            if index != path.len() - 1 {
                // If the found object is a directory, else if it is the final file return it
                if let Some(new_dir) = current_table.get_dir(obj) {
                    current_dir = Some(obj.clone());
                    current_table = FileTableSector::load(
                        new_dir.entry_addr,
                        self.drive_index as usize,
                        current_dir.clone(),
                    );
                } else {
                    return Some(current_table);
                }
            } else {
                return Some(current_table);
            }
        }

        None
    }

    /// Write a file to the given path containing the specified bytes.
    pub fn write_file(&mut self, path: &Vec<String>, bytes: Vec<u8>) -> ExitCode {
        let mut table_obj: FileTableSector;
        let mut table = &mut self.entry_table;

        for dir in &path[..path.len() - 1] {
            // Iterate over the tables representing the dir
            while table.get_dir(dir).is_none() {
                if let Some(new_addr) = table.continuation_addr {
                    table_obj = FileTableSector::load(
                        new_addr,
                        self.drive_index as usize,
                        table.directory_name.clone(),
                    );
                    table = &mut table_obj;
                } else {
                    return ExitCode::NotFoundError;
                }
            }

            if let Some(d) = table.get_dir(dir) {
                table_obj =
                    FileTableSector::load(d.entry_addr, self.drive_index as usize, Some(d.name));
                table = &mut table_obj;
            } else {
                return ExitCode::NotFoundError;
            }
        }

        let main_dir_name = table.directory_name.clone();

        while table.files.len() == 8 {
            if let Some(new_addr) = table.continuation_addr {
                table_obj = FileTableSector::load(
                    new_addr,
                    self.drive_index as usize,
                    main_dir_name.clone(),
                );
                table = &mut table_obj;
            } else {
                let drives = ata::DRIVES.lock();
                let drive = &drives[self.drive_index as usize];
                let new_sector = drive.find_available_sector().unwrap();
                drop(drives);

                table.set_continuation(new_sector);
                table_obj = FileTableSector::new(
                    new_sector,
                    self.drive_index as usize,
                    main_dir_name.clone(),
                );
                table = &mut table_obj;
            }
        }

        let drives = ata::DRIVES.lock();
        let new_file_sector = drives[self.drive_index as usize]
            .find_available_sector()
            .unwrap();

        drop(drives);

        table.add_file(&path[path.len() - 1], new_file_sector);

        let drives = ata::DRIVES.lock();
        let drive = &drives[self.drive_index as usize];

        let mut bytes_to_write = bytes.clone();
        bytes_to_write.truncate(506);
        let mut written_bytes = bytes_to_write.len();
        let mut current_sector = DataSector::new(new_file_sector, drive, bytes_to_write);

        while written_bytes < bytes.len() {
            bytes_to_write = bytes.clone();
            bytes_to_write.drain(..written_bytes);
            bytes_to_write.truncate(506);
            let extension_file_sector = drive.find_available_sector().unwrap();
            current_sector.continuation_addr = Some(extension_file_sector);
            current_sector.update_physical_drive(drive);
            written_bytes += bytes_to_write.len();
            current_sector = DataSector::new(extension_file_sector, drive, bytes_to_write);
        }

        ExitCode::Success
    }

    /// Create a directory at the given path.
    pub fn create_dir(&mut self, path: &Vec<String>) -> ExitCode {
        let mut table_obj: FileTableSector;
        let mut table = &mut self.entry_table;

        for dir in &path[..path.len() - 1] {
            // Iterate over the tables representing the dir
            while table.get_dir(dir).is_none() {
                if let Some(new_addr) = table.continuation_addr {
                    table_obj = FileTableSector::load(
                        new_addr,
                        self.drive_index as usize,
                        table.directory_name.clone(),
                    );
                    table = &mut table_obj;
                } else {
                    return ExitCode::NotFoundError;
                }
            }

            if let Some(d) = table.get_dir(dir) {
                table_obj =
                    FileTableSector::load(d.entry_addr, self.drive_index as usize, Some(d.name));
                table = &mut table_obj;
            } else {
                return ExitCode::NotFoundError;
            }
        }

        let main_dir_name = table.directory_name.clone();

        while table.files.len() == 8 {
            if let Some(new_addr) = table.continuation_addr {
                table_obj = FileTableSector::load(
                    new_addr,
                    self.drive_index as usize,
                    main_dir_name.clone(),
                );
                table = &mut table_obj;
            } else {
                let drives = ata::DRIVES.lock();
                let drive = &drives[self.drive_index as usize];
                let new_sector = drive.find_available_sector().unwrap();
                drop(drives);

                table.set_continuation(new_sector);
                table_obj = FileTableSector::new(
                    new_sector,
                    self.drive_index as usize,
                    main_dir_name.clone(),
                );
                table = &mut table_obj;
            }
        }

        let drives = ata::DRIVES.lock();
        let new_file_sector = drives[self.drive_index as usize]
            .find_available_sector()
            .unwrap();

        drop(drives);

        table.add_dir(&path[path.len() - 1], new_file_sector);
        FileTableSector::new(new_file_sector, self.drive_index as usize, None);

        ExitCode::Success
    }

    /// List the files at a given path.
    pub fn list_files(&self, path: &Vec<String>) -> Option<Vec<String>> {
        let mut result: Vec<String> = Vec::new();
        let mut table = self.entry_table.clone();

        for dir in path {
            // Iterate over the tables representing the dir
            while table.get_dir(dir).is_none() {
                if let Some(new_addr) = table.continuation_addr {
                    table = FileTableSector::load(
                        new_addr,
                        self.drive_index as usize,
                        table.directory_name,
                    );
                } else {
                    return None;
                }
            }

            if let Some(d) = table.get_dir(dir) {
                table =
                    FileTableSector::load(d.entry_addr, self.drive_index as usize, Some(d.name));
            } else {
                return None;
            }
        }

        result.extend(table.files.iter().map(|f| match f {
            FileType::File(f) => f.name.clone(),
            FileType::Dir(d) => format!("{}/", d.name),
        }));

        if table.continuation_addr.is_some() {
            let mut next_addr = table.continuation_addr.unwrap();
            loop {
                let table = FileTableSector::load(
                    next_addr,
                    self.drive_index as usize,
                    table.directory_name.clone(),
                );
                result.extend(table.files.iter().map(|f| match f {
                    FileType::File(f) => f.name.clone(),
                    FileType::Dir(d) => format!("{}/", d.name),
                }));
                if table.continuation_addr.is_some() {
                    next_addr = table.continuation_addr.unwrap();
                } else {
                    break;
                }
            }
        };

        Some(result)
    }

    /// Permanently delete a file from the disk.
    pub fn delete_file(&mut self, path: &Vec<String>) -> ExitCode {
        if let Some(file) = self.get_file(path) {
            let drives = ata::DRIVES.lock();
            let drive = &drives[self.drive_index as usize];
            let mut current_sector = DataSector::load(file.entry_addr, drive);
            let mut sectors_to_remove: Vec<DataSector> = Vec::new();

            loop {
                sectors_to_remove.push(current_sector.clone());
                if let Some(new_addr) = current_sector.continuation_addr {
                    current_sector = DataSector::load(new_addr, drive);
                } else {
                    break;
                }
            }

            for mut sector in sectors_to_remove {
                sector.remove(drive);
            }

            drop(drives);

            let mut file_table_sector = self.get_table_with_object(path).unwrap();
            let remove_index = file_table_sector
                .files
                .iter()
                .position(|ft| match ft {
                    FileType::File(f) => f.entry_addr == file.entry_addr,
                    FileType::Dir(_) => false,
                })
                .unwrap();

            file_table_sector.files.remove(remove_index);
            file_table_sector.update_physical_drive();

            self.entry_table =
                FileTableSector::load(self.entry_sector, self.drive_index as usize, None);

            ExitCode::Success
        } else {
            ExitCode::NotFoundError
        }
    }

    /// Permanently delete an empty directory from the disk.
    pub fn delete_dir(&mut self, path: &Vec<String>) -> ExitCode {
        if let Some(dir) = self.get_dir(path) {
            let mut current_sector =
                FileTableSector::load(dir.entry_addr, self.drive_index as usize, None);
            let mut sectors_to_remove: Vec<FileTableSector> = Vec::new();

            if current_sector.files.len() != 0 {
                return ExitCode::NotEmptyError;
            }

            loop {
                sectors_to_remove.push(current_sector.clone());
                if let Some(new_addr) = current_sector.continuation_addr {
                    current_sector =
                        FileTableSector::load(new_addr, self.drive_index as usize, None);
                } else {
                    break;
                }
            }

            for mut sector in sectors_to_remove {
                sector.remove();
            }

            let mut file_table_sector = self.get_table_with_object(path).unwrap();
            let remove_index = file_table_sector
                .files
                .iter()
                .position(|ft| match ft {
                    FileType::Dir(d) => d.entry_addr == dir.entry_addr,
                    FileType::File(_) => false,
                })
                .unwrap();

            file_table_sector.files.remove(remove_index);
            file_table_sector.update_physical_drive();

            self.entry_table =
                FileTableSector::load(self.entry_sector, self.drive_index as usize, None);

            ExitCode::Success
        } else {
            ExitCode::NotFoundError
        }
    }
}

/// Abstract struct representing a file, not connected in any way to disk
#[derive(Clone, Debug)]
pub struct File {
    pub name: String,
    pub drive_index: usize,
    pub entry_addr: u32,
}

impl File {
    /// Read bytes from the file, following the linked list.
    pub fn read(&self) -> Vec<u8> {
        let drives = ata::DRIVES.lock();
        let drive: &Drive = &drives[self.drive_index];

        let mut output_bytes: Vec<u8> = Vec::new();
        let mut current_addr = self.entry_addr;
        let mut current_sector = DataSector::load(current_addr, drive);

        loop {
            output_bytes.extend(
                current_sector.data[0..current_sector.size as usize]
                    .iter()
                    .cloned(),
            );
            if let Some(next_sector) = current_sector.continuation_addr {
                current_addr = next_sector;
                current_sector = DataSector::load(current_addr, drive);
            } else {
                break;
            }
        }

        output_bytes
    }
}

/// Abstract struct representing a directory, not connected in any way to disk.
#[derive(Clone, Debug)]
pub struct Dir {
    pub name: String,
    pub drive_index: usize,
    pub entry_addr: u32,
}

/// Represents a file type, either a file or directory.
#[derive(Clone)]
pub enum FileType {
    File(File), // File object
    Dir(Dir),   // Directory object
}

/// Represents a sector of the disk containing a file table.
#[derive(Clone)]
pub struct FileTableSector {
    pub addr: u32,
    pub directory_name: Option<String>,
    pub continuation_addr: Option<u32>,
    pub files: Vec<FileType>,
    pub drive_index: usize,
    pub is_deleted: bool,
}

impl FileTableSector {
    //// Load a `FileTableSector` object from its address
    pub fn load(addr: u32, drive_index: usize, directory_name: Option<String>) -> Self {
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
        let mut files: Vec<FileType> = Vec::new();

        let data_bytes = &buf[4..508]; // bytes 508 - 511 are ignored as they contain "POGO"
        for i in 0_usize..8 {
            let file_bytes = &data_bytes[i * 63..(i + 1) * 63];
            let file_name_bytes = &file_bytes[0..58];
            let file_type_byte = &file_bytes[58];
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
                if *file_type_byte == 0 {
                    files.push(FileType::File(File {
                        name: file_name,
                        entry_addr: file_addr,
                        drive_index,
                    }));
                } else {
                    files.push(FileType::Dir(Dir {
                        name: file_name,
                        entry_addr: file_addr,
                        drive_index,
                    }));
                }
            }
        }

        FileTableSector {
            addr,
            directory_name,
            continuation_addr: continuation_option,
            files,
            drive_index,
            is_deleted: false,
        }
    }

    /// Initialise a brand new sector on the disk, then return a virtual instance of it.
    pub fn new(new_addr: u32, drive_index: usize, directory_name: Option<String>) -> Self {
        let drive: &Drive = &ata::DRIVES.lock()[drive_index];

        let mut init_buf = [0_u8; 512];
        init_buf[508] = b'P';
        init_buf[509] = b'O';
        init_buf[510] = b'G';
        init_buf[511] = b'O';

        drive.write(new_addr, &init_buf);

        FileTableSector {
            addr: new_addr,
            directory_name,
            continuation_addr: None,
            files: Vec::new(),
            drive_index,
            is_deleted: false,
        }
    }

    /// Remove the sector from the disk.
    pub fn remove(&mut self) {
        self.continuation_addr = None;
        self.files = Vec::new();
        self.is_deleted = true;
        self.update_physical_drive();
    }

    /// Update the virtual parameters onto the disk.
    pub fn update_physical_drive(&self) {
        let drive: &Drive = &ata::DRIVES.lock()[self.drive_index];
        let mut buf = [0_u8; 512];

        if let Some(continuation) = self.continuation_addr {
            buf[0] = continuation.get_bits(24..32) as u8;
            buf[1] = continuation.get_bits(16..24) as u8;
            buf[2] = continuation.get_bits(8..16) as u8;
            buf[3] = continuation.get_bits(0..8) as u8;
        } else {
            buf[0] = 0;
            buf[1] = 0;
            buf[2] = 0;
            buf[3] = 0;
        }

        let mut index = 4;
        for file_type in &self.files {
            match file_type {
                FileType::File(file) => {
                    for (current_index, byte) in file.name.bytes().enumerate() {
                        buf[index + current_index] = byte;
                    }

                    buf[index + 59] = file.entry_addr.get_bits(24..32) as u8;
                    buf[index + 60] = file.entry_addr.get_bits(16..24) as u8;
                    buf[index + 61] = file.entry_addr.get_bits(8..16) as u8;
                    buf[index + 62] = file.entry_addr.get_bits(0..8) as u8;
                }
                FileType::Dir(dir) => {
                    for (current_index, byte) in dir.name.bytes().enumerate() {
                        buf[index + current_index] = byte;
                    }

                    buf[index + 58] = 0x01;
                    buf[index + 59] = dir.entry_addr.get_bits(24..32) as u8;
                    buf[index + 60] = dir.entry_addr.get_bits(16..24) as u8;
                    buf[index + 61] = dir.entry_addr.get_bits(8..16) as u8;
                    buf[index + 62] = dir.entry_addr.get_bits(0..8) as u8;
                }
            }

            index += 63;
        }

        if !self.is_deleted {
            buf[508] = b'P';
            buf[509] = b'O';
            buf[510] = b'G';
            buf[511] = b'O';
        }

        drive.write(self.addr, &buf);
    }

    /// Set the continuation address on disk
    pub fn set_continuation(&mut self, sector: u32) {
        self.continuation_addr = Some(sector);
        self.update_physical_drive();
    }

    /// Add a file to the table and update the physical drive.
    /// WARNING: This does not add the file to the disk, only a reference to the file on the table.
    /// WARNING: This does not create a new table if the current one is full.
    pub fn add_file(&mut self, name: &str, addr: u32) {
        assert!(self.files.len() < 8);
        self.files.push(FileType::File(File {
            name: name.to_owned(),
            drive_index: self.drive_index,
            entry_addr: addr,
        }));
        self.update_physical_drive();
    }

    /// Add a directory to the table and update the physical drive.
    /// WARNING: This does not add the directory to the disk, only a reference to the directory on the table.
    /// WARNING: This does not create a new table if the current one is full.
    pub fn add_dir(&mut self, name: &str, addr: u32) {
        assert!(self.files.len() < 8);
        self.files.push(FileType::Dir(Dir {
            name: name.to_owned(),
            drive_index: self.drive_index,
            entry_addr: addr,
        }));
        self.update_physical_drive();
    }

    /// Gets a specified file from the sector.
    /// If none is found, returns `None`.
    pub fn get_file(&self, name: &str) -> Option<File> {
        match self.files.iter().find(|el| match el {
            FileType::File(f) => f.name == name,
            _ => false,
        }) {
            Some(FileType::File(f)) => Some(f.clone()),
            _ => None,
        }
    }

    /// Gets a specified directory from the sector.
    /// If none is found, returns `None`.
    pub fn get_dir(&self, name: &str) -> Option<Dir> {
        match self.files.iter().find(|el| match el {
            FileType::Dir(d) => d.name == name,
            _ => false,
        }) {
            Some(FileType::Dir(d)) => Some(d.clone()),
            _ => None,
        }
    }

    /// Checks if sector contains a file or directory with the given name.
    /// Does not return the found object.
    pub fn contains_object(&self, name: &str) -> bool {
        self.files
            .iter()
            .find(|el| match el {
                FileType::File(f) => f.name == name,
                FileType::Dir(d) => d.name == name,
            })
            .is_some()
    }
}

/// Represents a sector of the disk containing data
#[derive(Clone)]
pub struct DataSector {
    pub addr: u32,
    pub continuation_addr: Option<u32>,
    pub size: u16,
    pub data: [u8; 506],
    pub drive_index: usize,
}

impl DataSector {
    /// Loads a new `DataSector` object from its address
    pub fn load(addr: u32, drive: &Drive) -> Self {
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
    pub fn new(addr: u32, drive: &Drive, bytes: Vec<u8>) -> Self {
        let mut buf = [0_u8; 512];
        let size = bytes.len() as u16;
        buf[4] = size.get_bits(8..16) as u8;
        buf[5] = size.get_bits(0..8) as u8;

        let index = 6;
        for (current_index, byte) in bytes.iter().enumerate() {
            buf[index + current_index] = *byte;
        }

        drive.write(addr, &buf);
        return Self::load(addr, drive);
    }

    /// Removes the sector from the disk.
    pub fn remove(&mut self, drive: &Drive) {
        self.continuation_addr = None;
        self.data = [0_u8; 506];
        self.size = 0;
        self.update_physical_drive(drive);
    }

    /// Updates the physical disk with the contents of the virtual sector.
    pub fn update_physical_drive(&self, drive: &Drive) {
        let mut buf = [0_u8; 512];
        drive.read(self.addr, &mut buf);

        if let Some(continuation) = self.continuation_addr {
            buf[0] = continuation.get_bits(24..32) as u8;
            buf[1] = continuation.get_bits(16..24) as u8;
            buf[2] = continuation.get_bits(8..16) as u8;
            buf[3] = continuation.get_bits(0..8) as u8;
        } else {
            buf[0] = 0;
            buf[1] = 0;
            buf[2] = 0;
            buf[3] = 0;
        }

        buf[4] = self.size.get_bits(8..16) as u8;
        buf[5] = self.size.get_bits(0..8) as u8;

        for index in 6_usize..512 {
            buf[index] = self.data[index - 6];
        }

        drive.write(self.addr, &buf);
    }
}

/// Create the basic filesystem on a drive specified by the user.
/// Allows the user to cancel at several points.
fn create_fs() {
    let drives = ata::DRIVES.lock();
    let mut filesystem = FILESYSTEM.lock();

    info(&format!("detected {} drive(s):\n", drives.len()));
    for drive in &*drives {
        println!(
            "         {}: {} {} ({} MB)",
            drive.drive_index,
            drive.model,
            drive.serial,
            drive.sectors / 2048
        );
    }

    println!();
    let mut drive_index = -1_i8;

    while drive_index < 0 || drive_index >= drives.len() as i8 {
        info("select a drive or type x to exit: ");
        let mut char_buf = [0_u8; 1];
        STDIN.get_char().encode_utf8(&mut char_buf);
        println!();
        match char_buf[0] {
            48..=57 => drive_index = (char_buf[0] - 48) as i8,
            120 => return warn("running in diskless mode, some features will be unavailable\n"),
            _ => continue,
        }
    }

    warn("this disk will be overwritten, continue? (y/n): ");
    let confirmation = STDIN.get_char();
    println!();
    if confirmation != 'y' {
        return warn("running in diskless mode, some features will be unavailable\n");
    }

    info(&format!("creating filesystem on disk {}\n", drive_index));

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
        drive_index: drive_index as u8,
        entry_sector: sectors - 1,
        entry_table: FileTableSector::load(sectors - 1, drive_index as usize, None),
    });

    okay("filesystem successfully created\n");
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
                    entry_table: FileTableSector::load(entry_sector, drive_index as usize, None),
                });
                break;
            }
        }
    }

    let filesystem = FILESYSTEM.lock();

    if let Some(fs) = &*filesystem {
        okay(&format!("filesystem detected on disk {}\n", fs.drive_index));
    } else {
        warn("no filesystem detected, initialise one now? (y/n): ");
        let char_input = STDIN.get_char();
        println!();
        if char_input == 'y' {
            drop(filesystem);
            create_fs();
        } else {
            warn("running in diskless mode, some features will be unavailable\n");
        }
    }
}

/// Checks if the filesystem is mounted.
pub fn is_mounted() -> bool {
    FILESYSTEM.lock().is_some()
}
