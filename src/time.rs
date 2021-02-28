use core::hint::spin_loop;

pub fn rdtsc() -> u64 {
    unsafe {
        core::arch::x86_64::_mm_lfence();
        core::arch::x86_64::_rdtsc()
    }
}

pub fn wait_nano(nanoseconds: u64) {
    let start = rdtsc();
    while rdtsc() - start < nanoseconds * 4 {
        spin_loop();
    }
}
