use core::ptr::NonNull;
use uefi::boot::*;
use uefi::proto::media::file::*;
use uefi::proto::media::fs::SimpleFileSystem;
use xmas_elf::ElfFile;

use super::{App, AppList};
use arrayvec::{ArrayString, ArrayVec};


/// Open root directory
pub fn open_root() -> Directory {
    let handle = uefi::boot::get_handle_for_protocol::<SimpleFileSystem>()
        .expect("Failed to get handle for SimpleFileSystem");
    let mut fs = uefi::boot::open_protocol_exclusive::<SimpleFileSystem>(handle)
        .expect("Failed to get FileSystem");

    fs.open_volume().expect("Failed to open volume")
}

/// Open file at `path`
pub fn open_file(path: &str) -> RegularFile {
    let mut buf = [0; 64];
    let cstr_path = uefi::CStr16::from_str_with_buf(path, &mut buf).unwrap();

    let handle = open_root()
        .open(cstr_path, FileMode::Read, FileAttribute::empty())
        .expect("Failed to open file");

    match handle.into_type().expect("Failed to into_type") {
        FileType::Regular(regular) => regular,
        _ => panic!("Invalid file type"),
    }
}

/// Load file to new allocated pages
pub fn load_file(file: &mut RegularFile) -> &'static mut [u8] {
    let mut info_buf = [0u8; 0x100];
    let info = file
        .get_info::<FileInfo>(&mut info_buf)
        .expect("Failed to get file info");

    let pages = info.file_size() as usize / 0x1000 + 1;

    let mem_start =
        uefi::boot::allocate_pages(AllocateType::AnyPages, MemoryType::LOADER_DATA, pages)
            .expect("Failed to allocate pages");

    let buf = unsafe { core::slice::from_raw_parts_mut(mem_start.as_ptr(), pages * 0x1000) };
    let len = file.read(buf).expect("Failed to read file");

    info!(
        "Load file \"{}\" to memory, size = {}",
        info.file_name(),
        len
    );

    &mut buf[..len]
}

/// Free ELF files for which the buffer was created using 'load_file'
pub fn free_elf(elf: ElfFile) {
    let buffer = elf.input;
    let pages = buffer.len() / 0x1000 + 1;
    let mem_start = NonNull::new(buffer.as_ptr() as *mut u8).expect("Invalid pointer");

    info!("Free ELF file, pages = {}, addr = {:#x?}", pages, mem_start);

    unsafe {
        uefi::boot::free_pages(mem_start, pages).expect("Failed to free pages");
    }
}

/// Load apps into memory, when no fs implemented in kernel
///
/// List all file under "APP" and load them.
pub fn load_apps() -> AppList {
    let mut root = open_root();
    let mut buf = [0; 8];
    let cstr_path: &uefi::CStr16 = uefi::CStr16::from_str_with_buf("\\APP\\", &mut buf).unwrap();

    // 1. get the file handle of the root directory
    let mut handle = open_root()
        .open(cstr_path, FileMode::Read, FileAttribute::empty())
        .expect("Failed to open file")
        .into_directory()
        .expect("Can't be a Directory");

    let mut apps = ArrayVec::new();
    let mut entry_buf = [0u8; 0x100];

    loop {
        // 2. get the struct containing information about a single file
        let info: Option<&mut FileInfo> = handle
            .read_entry(&mut entry_buf)
            .expect("Failed to read entry");
        // handle是已经打开的文件夹句柄，read_entry()用于遍历该目录内容，每次阅读一条
        // 返回的info类型为 Option<&FileInfo>，结构体定义如下
        // pub struct FileInfo {
        //     pub size: u64,          // 文件大小
        //     pub file_size: u64,     // 实际文件大小（可能和 size 不同）
        //     pub physical_size: u64, // 物理大小（占用空间）
        //     pub create_time: Time,  // 创建时间
        //     pub modify_time: Time,  // 修改时间
        //     pub attribute: FileAttribute, // 文件属性（如目录、隐藏等）
        //     pub file_name: [u16],   // 文件名（UTF-16 字符串）
        // }

        match info {
            Some(entry) => {
                // 3. open file with the name under current file handle
                let file = handle
                    .open(entry.file_name(), FileMode::Read, FileAttribute::empty())
                    .unwrap();

                // The type of `file` should be RegularFile instead of Dictory
                if file.is_directory().unwrap_or(true) {
                    continue;
                }

                let elf = {
                    // 4. load file with `load_file` function
                    //    check if the type of `file` is RegularFile 
                    let elf_file = load_file(file.into_regular_file().as_mut().unwrap());
                    // 5. convert file to `ElfFile`
                    ElfFile::new(elf_file).unwrap()
                };

                // 6. get the name of appliacation and push it into ArrayVec `apps`
                let mut name = ArrayString::<16>::new();
                entry.file_name().as_str_in_buf(&mut name).unwrap();

                apps.push(App { name, elf });
            }
            None => break,
        }
    }

    info!("Loaded {} apps", apps.len());

    apps
}
