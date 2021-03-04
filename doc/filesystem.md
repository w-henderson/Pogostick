# Pogostick Filesystem
The Pogostick filesystem (PFS) is loosely based on the FAT filesystem, as it uses linked list allocation with file tables to store data. PFS, however, is much simpler than FAT, as for such a small project, the additional features of FAT are unnecessary and would take an enormous amount of time and effort to implement. Assume all multi-bit values are stored as big endian.

## Master Sector
PFS uses the **last sector of the disk** as a so-called "master sector" instead of the first, as the bootloader and kernel are installed at the start of the disk. This master sector is the entry point to the root directory, and is formatted identically to any other directory sector, as discussed later. To detect a filesystem, Pogostick checks that the last 4 bytes of the last sector equal `POGO` in ASCII (indicating that it is a valid PFS directory sector).

## Directory Sectors / File Table Sectors
Directory sectors can hold information about 8 files/directories (referred to as objects) before another needs to be created and linked to. The first four bytes of the sector contain the sector number of the next sector in the linked list. If this address is `0x00000000`, the sector is treated as being the end of the linked list, with no further sectors. This is safe because the first sector of the disk will always contain the bootloader, so it could never hold a directory sector. Each of the eight objects contained within the sector has 58 bytes dedicated to the name in ASCII (`0x00` bytes are ignored completely), then 1 byte referring to the object type (`0x00` for file, `0x01` for directory). At the end of each directory sector are the characters `POGO` in ASCII, indicating that it is a valid PFS directory sector.

### Example Directory Sector Layout
Byte numbers are measured as the offset from the start of the sector. If a range is specified, it includes the first number and excludes the last number, as in Rust. In this example, the hard disk is 32 MB, but PFS supports hard disks up to 2 TB due to addressing sectors with a 32-bit unsigned integer.
| Byte(s) | Rust Type | Example Value | Meaning |
| --- | --- | --- | --- |
| `0x00..0x04` | `u32` | `0x0000FFA3` | The following directory sector for the same directory in the linked list can be found in sector `0x0000FFA3`. |
| `0x04..0x3e` | `[u8; 58]` | `example_file.txt` | The name of this object is `example_file.txt`. |
| `0x3e` | `u8` | `0x00` | This is a file object. |
| `0x3f..0x43` | `u32` | `0x0000FFE2` | This file's entry sector (formatted as a file sector as discussed later) can be found in sector `0x0000FFE2`. |
| `0x43..0x7d` | `[u8; 58]` | `example_dir` | The name of this object is `example_dir`. |
| `0x7d` | `u8` | `0x01` | This is a directory object. |
| `0x7e..0x82` | `u32` | `0x0000FF2B` | This directory's entry sector (formatted like this example) can be found in sector `0x0000FF2B`. |
| up to 8 object entries... |
| `0x01fc..0x0200` | `[u8; 4]` | `POGO` (always) | This is a valid PFS directory sector. |

## File Sectors
File sectors hold at least part of a file, as well as a reference to the sector containing the next part if the file is greater than 506 bytes. This is because only 506 out of the 512 bytes of the sector are able to store the actual file data, as the reference to the next part as well as the data size also have to be stored. Unlike with filenames in directory sectors, the size cannot be inferred as null bytes could be part of the file data. The four-byte continuation address is stored in the first four bytes of the sector, followed by two bytes indicating the size of the data in bytes. If the first four bytes are non-null, this indicates that the sector is not at the end of the linked list, and therefore all 506 bytes of the data are required. This will be reflected in that the two size bytes will equal `0x01fa`.

### Example File Sector Layout
| Byte(s) | Rust Type | Example Value | Meaning |
| --- | --- | --- | --- |
| `0x00..0x04` | `u32` | `0x0000FFA0` | The following file sector for this individual file can be found in sector `0x0000FFA0`. |
| `0x04..0x06` | `u16` | `0x01fa` | `0x01fa` of the following data bytes are in use (in this case, all of them). |
| `0x0006..0x0200` | `[u8; 506]` | any | Part or the entirety of the data for this file. |

# Interacting with PFS within Pogostick
Pogostick's integration with the PFS is still limited, as is the filesystem itself. You can currently traverse directories with the `cd` command, create text files with `wt`, read text files with `rt`, create directories with `mkdir`, and list directories with `ls` or `dir` at your choosing. There is currently no way of deleting files, although this is being developed.

```
pogo:$~/ mkdir example_dir

pogo:$~/ dir
- example_dir/

pogo:$~/ cd example_dir

pogo:$~/example_dir/ wt file_name.txt hello world

pogo:$~/example_dir/ ls
- file_name

pogo:$~/example_dir/ rt file_name.txt
hello world

pogo:$~/example_dir/ cd ..

pogo:$~/ rt example_dir/file_name.txt
hello world
```

# Glossary
| Term | Definition |
| --- | --- |
| Master sector | The last sector of the disk containing the entry directory sector for the root directory. |
| Directory sector | A sector containing the names and addresses of files and directories within a directory. |
| File sector | A sector containing part or all of the data for a file. |
| Continuation address | The address of the next part of the same directory or file. |
| Object | In this context, a file or directory. |
| Entry sector | The first sector for an object, often linking to subsequent sectors. |
| Root directory | The top-level directory of the filesystem, represented by a `/`. |