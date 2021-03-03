use crate::input::STDIN;
use crate::println;
use crate::time::Time;
use crate::vga::{err, info, okay, warn, Colour, ColourCode, BUFFER_HEIGHT, WRITER};
use alloc::{
    borrow::ToOwned,
    boxed::Box,
    format,
    string::{String, ToString},
    vec::Vec,
};
use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::instructions::interrupts;

lazy_static! {
    pub static ref PATH: Mutex<Vec<String>> = Mutex::new(Vec::new());
}

/// Provide a console input forever
pub fn console_loop() -> ! {
    info(&format!(
        "boot completed at {}\n\n",
        Time::get().to_string()
    ));

    let prompt_colour = ColourCode::new(Colour::LightGreen, Colour::Black);
    let path_colour = ColourCode::new(Colour::LightCyan, Colour::Black);

    let lock_write_colour = |text: &str, colour: ColourCode| {
        interrupts::without_interrupts(|| {
            WRITER.lock().write_string_colour(text, colour);
        });
    };

    loop {
        let path_lock = PATH.lock();
        let path = path_lock.clone();
        let mut path_display = path.iter().fold(String::from("/"), |mut acc, x| {
            acc.extend(x.chars());
            acc.push('/');
            acc
        });
        path_display.push(' ');
        drop(path_lock);

        lock_write_colour("pogo:$~", prompt_colour);
        lock_write_colour(&path_display, path_colour);
        let command_str = STDIN.get_str();
        let command_split: Vec<&str> = command_str.split(" ").collect();
        let command: Box<dyn Command> = match command_split[0] {
            "cd" => CDCommand::new(&command_split[1..]),
            "echo" => Echo::new(&command_split[1..]),
            "clear" => ClearCommand::new(&[]),
            "add" => AddCommand::new(&command_split[1..]),
            "disk" => DiskInfoCommand::new(&[]),
            "ls" | "dir" => ListFilesCommand::new(&[]),
            "mkdir" => CreateDirCommand::new(&command_split[1..]),
            "wt" => WriteCommand::new(&command_split[1..]),
            "rt" => ReadCommand::new(&command_split[1..]),
            "time" => TimeCommand::new(&[]),
            _ => NullCommand::new(&[]),
        };

        let status_code = command.execute();
        match status_code {
            1 => err("generic command failure\n\n"),
            2 => err("filesystem not mounted\n\n"),
            255 => err("command not found\n\n"),
            _ => {
                println!();
                0
            }
        };
    }
}

trait Command {
    /// Create command from arguments.
    fn new(args: &[&str]) -> Box<Self>
    where
        Self: Sized;

    /// Execute command, returning status code.
    /// Status codes are 0 for success, 1 for generic error, 2 for filesystem error, and 255 for command not found.
    fn execute(&self) -> u8;
}

/// Basic echo command, prints its input to the output
struct Echo {
    pub text: String,
}

impl Command for Echo {
    fn new(args: &[&str]) -> Box<Self>
    where
        Self: Sized,
    {
        Box::new(Echo {
            text: args.join(" "),
        })
    }
    fn execute(&self) -> u8 {
        println!("{}", self.text);
        0
    }
}

/// Change directory command
struct CDCommand {
    pub new_dir: String,
}

impl Command for CDCommand {
    fn new(args: &[&str]) -> Box<Self> {
        Box::new(CDCommand {
            new_dir: args[0].to_string(),
        })
    }
    fn execute(&self) -> u8 {
        let filesystem = crate::fs::FILESYSTEM.lock();
        if let Some(fs) = filesystem.as_ref() {
            let mut new_dir = self.new_dir.clone();
            if new_dir.chars().nth(0) == Some('/') {
                new_dir.remove(0);
            }
            if new_dir.chars().last() == Some('/') {
                new_dir.pop();
            }

            let mut prospective_path = PATH.lock().clone();
            if new_dir == "" {
                prospective_path = Vec::new();
            } else if new_dir == ".." {
                prospective_path.pop();
            } else {
                prospective_path.extend(new_dir.split("/").map(|s| s.to_owned()));
            }

            if fs.list_files(&prospective_path).is_some() {
                *PATH.lock() = prospective_path.clone();
                0
            } else {
                1
            }
        } else {
            2
        }
    }
}

/// Command to clear the screen
struct ClearCommand;

impl Command for ClearCommand {
    fn new(_args: &[&str]) -> Box<Self> {
        Box::new(ClearCommand)
    }
    fn execute(&self) -> u8 {
        interrupts::without_interrupts(|| {
            for _ in 0..BUFFER_HEIGHT {
                WRITER.lock().new_line();
            }
        });
        0
    }
}

/// Command to get the current time
struct TimeCommand;

impl Command for TimeCommand {
    fn new(_args: &[&str]) -> Box<Self> {
        Box::new(TimeCommand)
    }
    fn execute(&self) -> u8 {
        println!("{}", Time::get().to_string());
        0
    }
}

/// Command to add two numbers
struct AddCommand {
    number1: f64,
    number2: f64,
    parse_error: bool,
}

impl Command for AddCommand {
    fn new(args: &[&str]) -> Box<Self> {
        if args.len() != 2 {
            return Box::new(AddCommand {
                number1: 0_f64,
                number2: 0_f64,
                parse_error: true,
            });
        }

        let parsed_number1 = args[0].parse::<f64>();
        let parsed_number2 = args[1].parse::<f64>();
        if parsed_number1.is_ok() && parsed_number2.is_ok() {
            Box::new(AddCommand {
                number1: parsed_number1.unwrap(),
                number2: parsed_number2.unwrap(),
                parse_error: false,
            })
        } else {
            Box::new(AddCommand {
                number1: 0_f64,
                number2: 0_f64,
                parse_error: true,
            })
        }
    }
    fn execute(&self) -> u8 {
        if !self.parse_error {
            println!("{}", self.number1 + self.number2);
            0
        } else {
            1
        }
    }
}

// Command to list connected disks
struct DiskInfoCommand;

impl Command for DiskInfoCommand {
    fn new(_args: &[&str]) -> Box<Self> {
        Box::new(DiskInfoCommand)
    }
    fn execute(&self) -> u8 {
        let drives = crate::ata::DRIVES.lock();
        for drive in &*drives {
            info(&format!(
                "ATA {}: {} {} {} ({} MB)\n",
                drive.bus_index,
                drive.drive_index,
                drive.model,
                drive.serial,
                drive.sectors / 2048
            ));
        }
        0
    }
}

/// Command to list files
struct ListFilesCommand;

impl Command for ListFilesCommand {
    fn new(_args: &[&str]) -> Box<Self> {
        Box::new(ListFilesCommand)
    }
    fn execute(&self) -> u8 {
        let mut fs = crate::fs::FILESYSTEM.lock();
        let path = PATH.lock().clone();
        if let Some(filesystem) = fs.as_mut() {
            let files = filesystem.list_files(&path).unwrap();
            if files.len() == 0 {
                println!("no files in this directory");
                return 0;
            }
            for file in files {
                println!(" - {}", file);
            }
            0
        } else {
            2
        }
    }
}

/// Command to write text to a file
struct WriteCommand {
    name: String,
    text: String,
}

impl Command for WriteCommand {
    fn new(args: &[&str]) -> Box<Self> {
        Box::new(WriteCommand {
            name: args[0].to_owned(),
            text: args[1..].join(" "),
        })
    }
    fn execute(&self) -> u8 {
        let mut fs = crate::fs::FILESYSTEM.lock();
        let mut path = PATH.lock().clone();
        path.extend(self.name.split("/").map(|s| s.to_owned()));
        if let Some(filesystem) = fs.as_mut() {
            match filesystem.write_file(&path, self.text.as_bytes().to_vec()) {
                0 => okay("successfully written file\n"),
                error_code => error_code,
            }
        } else {
            2
        }
    }
}

/// Command to read text from a file
struct ReadCommand {
    name: String,
}

impl Command for ReadCommand {
    fn new(args: &[&str]) -> Box<Self> {
        Box::new(ReadCommand {
            name: args[0].to_owned(),
        })
    }
    fn execute(&self) -> u8 {
        let mut fs = crate::fs::FILESYSTEM.lock();
        let mut path = PATH.lock().clone();
        path.extend(self.name.split("/").map(|s| s.to_owned()));

        if let Some(filesystem) = fs.as_mut() {
            let file = filesystem.get_file(&path);

            if let Some(f) = file {
                let file_bytes = f.read();
                if let Ok(file_text) = core::str::from_utf8(&file_bytes) {
                    println!("{}", file_text)
                } else {
                    warn("cannot detect encoding, printing as hex\n\n");
                    println!("{}", hex::encode(file_bytes));
                }
                0
            } else {
                1
            }
        } else {
            2
        }
    }
}

/// Create directory command
struct CreateDirCommand {
    name: String,
}

impl Command for CreateDirCommand {
    fn new(args: &[&str]) -> Box<Self> {
        Box::new(CreateDirCommand {
            name: args[0].to_owned(),
        })
    }
    fn execute(&self) -> u8 {
        let mut fs = crate::fs::FILESYSTEM.lock();
        let mut path = PATH.lock().clone();
        path.push(self.name.clone());

        if let Some(filesystem) = fs.as_mut() {
            filesystem.create_dir(&path)
        } else {
            2
        }
    }
}

/// Null command, represents a non-existant command
struct NullCommand;

impl Command for NullCommand {
    fn new(_args: &[&str]) -> Box<Self> {
        Box::new(NullCommand)
    }
    fn execute(&self) -> u8 {
        255
    }
}
