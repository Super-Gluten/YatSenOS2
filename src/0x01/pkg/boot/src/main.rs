#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

#[macro_use]
extern crate log;
extern crate alloc;

use alloc::boxed::Box;
use alloc::vec;
use elf::load_elf;
use elf::map_physical_memory;
use elf::map_range;
use uefi::mem::memory_map::MemoryMap;
use uefi::{Status, entry};
use x86_64::registers::control::*;
use xmas_elf::ElfFile;
use ysos_boot::*;

mod config;
use config::Config;

const CONFIG_PATH: &str = "\\EFI\\BOOT\\boot.conf";

#[entry]
fn efi_main() -> Status {
    uefi::helpers::init().expect("Failed to initialize utilities");

    log::set_max_level(log::LevelFilter::Info);
    info!("Running UEFI bootloader...");

    // 1. Load config
    let config = {
        /* FIXME: Load config file */
        // 读取config路径并加载它
        let mut file = open_file(CONFIG_PATH);
        let buf = load_file(&mut file);

        crate::Config::parse(buf)
    };

    info!("Config: {:#x?}", config);

    // 2. Load ELF files
    let elf = {
        /* FIXME: Load kernel elf file */
        // 从 config.rs中读取内核存储地址
        let path = config.kernel_path;
        // 读取内核文件
        let mut file = open_file(path);
        let buf = load_file(&mut file);
        // 新建 ElfFile 结构体
        ElfFile::new(buf).unwrap()
    };

    // 0x04 加载用户程序
    let apps = if config.load_apps {
        info!("Loading apps...");
        Some(load_apps())
    } else {
        info!("Skip loading apps");
        None
    };

    unsafe {
        set_entry(elf.header.pt2.entry_point() as usize);
    }

    // 3. Load MemoryMap
    let mmap = uefi::boot::memory_map(MemoryType::LOADER_DATA).expect("Failed to get memory map");

    let max_phys_addr = mmap
        .entries()
        .map(|m| m.phys_start + m.page_count * 0x1000)
        .max()
        .unwrap()
        .max(0x1_0000_0000); // include IOAPIC MMIO area

    // 4. Map ELF segments, kernel stack and physical memory to virtual memory
    let mut page_table = current_page_table();

    // FIXME: root page table is readonly, disable write protect (Cr0)
    unsafe {
        // 使用remove去除Cr0的写保护
        Cr0::update(|f| f.remove(Cr0Flags::WRITE_PROTECT));
    }

    // FIXME: map physical memory to specific virtual address offset
    // 使用allocator.rs 中定义好的结构体UEFIFrameAllocator实现x86 trait
    // 并且使用elf的lib.rs中的map_physical_memory映射页表
    let mut frame_allocator = UEFIFrameAllocator;
    map_physical_memory(
        config.physical_memory_offset,
        max_phys_addr,
        &mut page_table,
        &mut frame_allocator,
    );
    // FIXME: load and map the kernel elf file
    // 使用填充完毕的load_segments函数
    // 由于load_elf已通过elf.program_iter()遍历并传递了参数，会自动处理segment参数，无需手动提供
    load_elf(
        &elf,
        config.physical_memory_offset,
        &mut page_table,
        &mut frame_allocator,
    );

    // FIXME: map kernel stack
    // 由于 config中定义kernel_stack_auto_grow = 0, 即栈不会自动增长, 直接定义
    // 然后使用elf中lib.rs中的map_range函数运行内核
    let (stack_start_address, stack_size) = (config.kernel_stack_address, config.kernel_stack_size);
    map_range(
        stack_start_address,
        stack_size,
        &mut page_table,
        &mut frame_allocator,
    );

    // FIXME: recover write protect (Cr0)
    unsafe {
        // 使用 insert还原Cr0的写保护
        Cr0::update(|f| f.insert(Cr0Flags::WRITE_PROTECT));
    }
    free_elf(elf);

    // 5. Pass system table to kernel
    let ptr = uefi::table::system_table_raw().expect("Failed to get system table");
    let system_table = ptr.cast::<core::ffi::c_void>();

    // 6. Exit boot and jump to ELF entry
    info!("Exiting boot services...");

    let mmap = unsafe { uefi::boot::exit_boot_services(MemoryType::LOADER_DATA) };
    // NOTE: alloc & log are no longer available

    // construct BootInfo
    let bootinfo = BootInfo {
        memory_map: mmap.entries().copied().collect(),
        physical_memory_offset: config.physical_memory_offset,
        system_table,
        loaded_apps : apps, // 0x04 将上文加载的用户程序信息传递给内核
    };

    // align stack to 8 bytes
    let stacktop = config.kernel_stack_address + config.kernel_stack_size * 0x1000 - 8;

    jump_to_entry(&bootinfo, stacktop);
}
