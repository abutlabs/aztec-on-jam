//! Spike (#62 step 2): Barretenberg 6.0.0's CHONK verifier, its C++ unchanged, compiled for the
//! PVM (../build.sh) and called from refine through ../shim.cpp's `chonk_pvm_verify`.
//!
//! Payload: [mode u8: 0 the per-proof reduction, 1 the whole verification][tag u8][the shim's
//! input: proof, vk and CRS points, as ../payload.py writes it].
//! Refine outputs [tag][verdict: 1 valid, 0 invalid, 2 malformed][gas the verifier used, u64 LE];
//! accumulate stores it at "v:" ++ tag.
//!
//! The C and C++ runtime the verifier needs comes from here (allocation, `abort`, `chonk_trap`),
//! ../rt.cpp and the generated stubs. The heap is jam-pvm-common's bump allocator, so `free`
//! reclaims nothing: a refine is short-lived, and the gas is the same either way (the GP does not
//! charge memory by the page). A stub the verifier reaches logs its name (JIP-1's log, shown by
//! lasair under LASAIR_DEBUG) and stops the refine.
#![no_std]
#![no_main]

extern crate alloc;

use alloc::alloc::{alloc as raw_alloc, Layout};
use jam_pvm_common::accumulate::{accumulate_items, set_storage};
use jam_pvm_common::jam_types::*;
use jam_pvm_common::refine::gas;
use jam_pvm_common::{declare_service, Service};

polkavm_derive::min_stack_size!(8 * 1024 * 1024);

extern "C" {
    fn chonk_pvm_verify(input: *const u8, len: usize, mode: i32) -> i32;
    // bb's static constructors, called once (../build.sh: the PVM never runs .init_array).
    fn chonk_pvm_init();
}

#[polkavm_derive::polkavm_import]
extern "C" {
    // JIP-1's log (host call 100): GP 0.8.0 charges it as an unknown host call.
    #[polkavm_import(index = 100)]
    fn log(level: u64, target_ptr: *const u8, target_len: u64, text_ptr: *const u8, text_len: u64);
}

/// A runtime symbol bb's verifier was not expected to reach: name it, then stop.
#[no_mangle]
pub unsafe extern "C" fn chonk_trap(what: *const u8) -> ! {
    let mut n = 0;
    while *what.add(n) != 0 {
        n += 1;
    }
    let target = b"chonk-pvm missing";
    log(0, target.as_ptr(), target.len() as u64, what, n as u64);
    panic!()
}

// Every block keeps its size in the 8 bytes before it, for realloc.
const HEADER: usize = 16;

unsafe fn block(size: usize, align: usize) -> *mut u8 {
    let align = align.max(HEADER);
    let Some(total) = size.checked_add(align + HEADER) else { return core::ptr::null_mut() };
    let base = raw_alloc(Layout::from_size_align_unchecked(total, HEADER));
    if base.is_null() {
        return base;
    }
    let p = (base as usize + HEADER).next_multiple_of(align) as *mut u8;
    *(p.sub(8) as *mut usize) = size;
    p
}

#[no_mangle]
pub unsafe extern "C" fn malloc(size: usize) -> *mut u8 {
    block(size, HEADER)
}

#[no_mangle]
pub unsafe extern "C" fn aligned_alloc(align: usize, size: usize) -> *mut u8 {
    block(size, align)
}

#[no_mangle]
pub unsafe extern "C" fn posix_memalign(out: *mut *mut u8, align: usize, size: usize) -> i32 {
    let p = block(size, align);
    *out = p;
    if p.is_null() { 12 } else { 0 }
}

#[no_mangle]
pub unsafe extern "C" fn calloc(n: usize, size: usize) -> *mut u8 {
    let Some(total) = n.checked_mul(size) else { return core::ptr::null_mut() };
    let p = block(total, HEADER);
    if !p.is_null() {
        core::ptr::write_bytes(p, 0, total);
    }
    p
}

#[no_mangle]
pub unsafe extern "C" fn realloc(old: *mut u8, size: usize) -> *mut u8 {
    let p = block(size, HEADER);
    if !old.is_null() && !p.is_null() {
        let was = *(old.sub(8) as *const usize);
        core::ptr::copy_nonoverlapping(old, p, was.min(size));
    }
    p
}

#[no_mangle]
pub extern "C" fn free(_p: *mut u8) {}

#[no_mangle]
pub extern "C" fn abort() -> ! {
    panic!()
}

declare_service!(Spike);
struct Spike;

impl Service for Spike {
    fn refine(_core: CoreIndex, _item: usize, _service: ServiceId, payload: WorkPayload, _pkg: WorkPackageHash) -> WorkOutput {
        let p = payload.take();
        let (mode, tag) = (p.first().copied().unwrap_or(0), p.get(1).copied().unwrap_or(0));
        let input = p.get(2..).unwrap_or(&[]);
        let before = gas();
        unsafe { chonk_pvm_init() };
        let v = unsafe { chonk_pvm_verify(input.as_ptr(), input.len(), mode as i32) };
        let used = before - gas();
        let mut out = alloc::vec![tag, match v { 1 => 1, 0 => 0, _ => 2 }];
        out.extend_from_slice(&used.to_le_bytes());
        out.into()
    }

    fn accumulate(_slot: Slot, _service: ServiceId, _items: usize) -> Option<Hash> {
        for item in accumulate_items() {
            if let AccumulateItem::WorkItem(rec) = item {
                if let Ok(out) = rec.result {
                    if let Some(&tag) = out.0.first() {
                        set_storage(&[b'v', b':', tag], &out.0[1..]).ok();
                    }
                }
            }
        }
        None
    }
}
