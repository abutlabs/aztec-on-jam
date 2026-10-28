//! Spike (#62 step 2): Barretenberg 6.0.0's CHONK verifier, its C++ unchanged, compiled for the
//! PVM (../build.sh) and called from refine through ../shim.cpp's `chonk_pvm_verify`.
//!
//! Payload: [mode u8: 0 the per-proof reduction, 1 the whole verification][tag u8][the shim's
//! input: proof, vk and CRS points, as ../payload.py writes it]. On a net, where a package is held
//! to 200 KiB and the 2^15 Grumpkin CRS points alone are 2 MiB, the payload is [mode][tag] and
//! the shim's input is the work-item's first extrinsic, which travels beside the package.
//! Refine outputs [tag][verdict: 1 valid, 0 invalid, 2 malformed][gas the verifier used, u64 LE]
//! [bb's Grumpkin MSMs done by call 105, u64 LE]; accumulate stores it at "v:" ++ tag.
//!
//! The CRS as a preimage (#73): with mode's bit 0x80 set, the extrinsic carries only the proof and
//! vk ([u32 len][proof][u32 len][vk], ../pv.py) and the CRS comes from the service's own preimage,
//! `CRS_HASH`, in bb's in-memory form (../crs_raw.cpp): fetched once by the chain, not 2 MiB in
//! every item, and copied rather than parsed. Its hash is fixed here, so no other CRS can stand in.
//! A work item of mode 0x40 asks accumulate to solicit that preimage; anyone may then provide it.
//! Without it, a 0x80 item's verdict is 3: the CRS is not available yet.
//!
//! bb's Grumpkin MSM goes to the proposed host call 105 where the net has it (../gk105.cpp,
//! through `curve_calls`' probe), and runs as PVM code where not: the same blob runs on both.
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
use jam_pvm_common::accumulate::{accumulate_items, set_storage, solicit};
use jam_pvm_common::jam_types::*;
use jam_pvm_common::refine::{extrinsic, gas, lookup};
use jam_pvm_common::{declare_service, Service};

polkavm_derive::min_stack_size!(8 * 1024 * 1024);

/// blake2b-256 of the CRS in bb's in-memory form: 1 BN254 point, the G2 point and 2^15 Grumpkin
/// points (../crs_raw.cpp from bb 6.0.0's CRS files, 2,097,352 octets).
const CRS_HASH: [u8; 32] = hex32("d4d4d86aec1285ed609d2651b761d25ae0fc72a5ec11dbb11e3e6cf740621196");
const CRS_LEN: usize = 2_097_352;
const FROM_PREIMAGE: u8 = 0x80;
const SOLICIT: u8 = 0x40;

const fn nibble(c: u8) -> u8 {
    if c <= b'9' { c - b'0' } else { c - b'a' + 10 }
}

const fn hex32(s: &str) -> [u8; 32] {
    let b = s.as_bytes();
    let mut out = [0u8; 32];
    let mut i = 0;
    while i < 32 {
        out[i] = nibble(b[2 * i]) << 4 | nibble(b[2 * i + 1]);
        i += 1;
    }
    out
}

extern "C" {
    fn chonk_pvm_verify(input: *const u8, len: usize, mode: i32) -> i32;
    fn chonk_pvm_verify_crs(input: *const u8, len: usize, crs: *const u8, crs_len: usize, mode: i32) -> i32;
    // bb's static constructors, called once (../build.sh: the PVM never runs .init_array).
    fn chonk_pvm_init();
    // how many of bb's Grumpkin MSMs call 105 did (../gk105.cpp)
    fn chonk_gk_calls() -> u64;
}

/// Call 105 for ../gk105.cpp: 0 the sum written to `out` (64 octets), 1 the call is absent (bb's
/// own MSM runs), 2 a malformed input or a broken host.
#[no_mangle]
pub unsafe extern "C" fn chonk_gk_msm(input: *const u8, len: usize, out: *mut u8) -> i32 {
    match curve_calls::grumpkin_g1_msm(core::slice::from_raw_parts(input, len)) {
        curve_calls::Answer::Value(o) => {
            core::ptr::copy_nonoverlapping(o.as_ptr(), out, 64);
            0
        }
        curve_calls::Answer::Absent => 1,
        _ => 2,
    }
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
        if mode == SOLICIT {
            return alloc::vec![tag, 4].into();
        }
        let ext;
        let input = match p.get(2..) {
            Some(rest) if !rest.is_empty() => rest,
            _ => { ext = extrinsic(0).unwrap_or_default(); &ext[..] }
        };
        let before = gas();
        let v = if mode & FROM_PREIMAGE != 0 {
            match lookup(&CRS_HASH) {
                Some(crs) if crs.len() == CRS_LEN => {
                    unsafe { chonk_pvm_init() };
                    unsafe { chonk_pvm_verify_crs(input.as_ptr(), input.len(), crs.as_ptr(), crs.len(), (mode & 1) as i32) }
                }
                _ => 3,
            }
        } else {
            unsafe { chonk_pvm_init() };
            unsafe { chonk_pvm_verify(input.as_ptr(), input.len(), mode as i32) }
        };
        let used = before - gas();
        let mut out = alloc::vec![tag, match v { 1 => 1, 0 => 0, 3 => 3, _ => 2 }];
        out.extend_from_slice(&used.to_le_bytes());
        out.extend_from_slice(&unsafe { chonk_gk_calls() }.to_le_bytes());
        out.into()
    }

    fn accumulate(_slot: Slot, _service: ServiceId, _items: usize) -> Option<Hash> {
        for item in accumulate_items() {
            if let AccumulateItem::WorkItem(rec) = item {
                if let Ok(out) = rec.result {
                    if out.0.len() == 2 && out.0[1] == 4 {
                        // mode 0x40: ask for the CRS preimage (its deposit comes from this service's balance)
                        let ok = solicit(&CRS_HASH, CRS_LEN).is_ok();
                        set_storage(b"crs:solicited", &[ok as u8]).ok();
                    } else if let Some(&tag) = out.0.first() {
                        set_storage(&[b'v', b':', tag], &out.0[1..]).ok();
                    }
                }
            }
        }
        None
    }
}
