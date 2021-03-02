use core::num::ParseFloatError;

use crate::input::STDIN;
use crate::println;
use crate::vga::{Colour, ColourCode, BUFFER_HEIGHT, WRITER};
use alloc::{borrow::ToOwned, boxed::Box, string::String, vec::Vec};
use x86_64::instructions::interrupts;

/// Provide a console input forever
pub fn console_loop() -> ! {
    let prompt_colour = ColourCode::new(Colour::LightGreen, Colour::Black);
    let error_colour = ColourCode::new(Colour::LightRed, Colour::Black);

    let lock_write_colour = |text: &str, colour: ColourCode| {
        interrupts::without_interrupts(|| {
            WRITER.lock().write_string_colour(text, colour);
        });
    };

    loop {
        lock_write_colour("pogo:$ ", prompt_colour);
        let command_str = STDIN.get_str();
        let command_split: Vec<&str> = command_str.split(" ").collect();
        let command: Box<dyn Command> = match command_split[0] {
            "echo" => Echo::new(&command_split[1..]),
            "clear" => ClearCommand::new(&[]),
            "add" => AddCommand::new(&command_split[1..]),
            "disk" => DiskInfoCommand::new(&[]),
            "ls" => ListFilesCommand::new(&[]),
            "write" => WriteCommand::new(&command_split[1..]),
            _ => NullCommand::new(&[]),
        };

        let status_code = command.execute();
        match status_code {
            1 => lock_write_colour("error: generic command failure\n\n", error_colour),
            255 => lock_write_colour("error: command not found\n\n", error_colour),
            _ => println!(),
        }
    }
}

trait Command {
    /// Create command from arguments.
    fn new(args: &[&str]) -> Box<Self>
    where
        Self: Sized;

    /// Execute command, returning status code.
    /// Status codes are 0 for success, 1 for generic error, and 255 for command not found.
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
            println!(
                "ATA {}: {} {} {} ({} MB)",
                drive.bus_index,
                drive.drive_index,
                drive.model,
                drive.serial,
                drive.sectors / 2048
            );
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
        let filesystem = fs.as_mut().unwrap();
        let files = filesystem.list_files();
        if files.len() == 0 {
            return 1;
        }
        for file in files {
            println!(" - {}", file);
        }
        0
    }
}

/// Command to write text to a file
struct WriteCommand {
    path: String,
    text: String,
}

impl Command for WriteCommand {
    fn new(args: &[&str]) -> Box<Self> {
        Box::new(WriteCommand {
            path: args[0].to_owned(),
            text: args[1..].join(" "),
        })
    }
    fn execute(&self) -> u8 {
        let mut fs = crate::fs::FILESYSTEM.lock();
        let filesystem = fs.as_mut().unwrap();
        filesystem.write_file(&self.path, self.text.as_bytes().to_vec());
        println!("successfully written file");
        0
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
