// GLOBAL DESCRIPTOR TABLE
// Loads task state segment holding interrupt stack table.
// This, I believe, allows us to use a different bit of memory for handling exceptions.
// This means that we won't get stuck in a boot loop if an exception occurs and we can't handle it because we're out of memory.

use lazy_static::lazy_static;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

struct Selectors {
    code_selector: SegmentSelector,
    tss_selector: SegmentSelector,
    data_selector: SegmentSelector,
}

lazy_static! {
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
            const STACK_SIZE: usize = 4096 * 5;
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

            let stack_start = VirtAddr::from_ptr(unsafe { &STACK });
            let stack_end = stack_start + STACK_SIZE;
            stack_end
        };
        tss
    };
}

lazy_static! {
    static ref GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();
        let code_selector = gdt.add_entry(Descriptor::kernel_code_segment());
        let data_selector = gdt.add_entry(Descriptor::kernel_data_segment());
        let tss_selector = gdt.add_entry(Descriptor::tss_segment(&TSS));
        (
            gdt,
            Selectors {
                code_selector,
                data_selector,
                tss_selector,
            },
        )
    };
}

pub fn init() {
    use x86_64::instructions::{segmentation, tables};

    GDT.0.load();
    unsafe {
        segmentation::set_cs(GDT.1.code_selector);
        segmentation::load_ds(GDT.1.data_selector);
        segmentation::load_es(GDT.1.data_selector);
        segmentation::load_fs(GDT.1.data_selector);
        segmentation::load_gs(GDT.1.data_selector);
        segmentation::load_ss(GDT.1.data_selector);
        tables::load_tss(GDT.1.tss_selector);
    }
}
