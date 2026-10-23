//! Spike: Poseidon2 in the PVM (#41). Payload [tag][kind][...][expected 32]:
//!   kind 0: [n u16] n chained hashes x <- H(x, x) from x = 1
//!   kind 1: [n u16][leaf 32][index u64][40 × sibling 32] the depth-40 root, n times (a tree
//!           insertion rehashes one path: 40 hashes)
//!   kind 2: [k u16][next index u64][40 × frontier 32][40 × zero 32][k × leaf 32] the depth-40
//!           root after appending k leaves to an append-only tree at the next index (#43).
//!           frontier[l] is the left node at level l the next index pairs with; zero[l] is the
//!           empty subtree's root at level l (a real service holds these as constants). Level by
//!           level, so k leaves cost about 2k + 40 hashes, not 40k.
//! Refine outputs [tag][1 the result equals the expected value | 0]; accumulate stores it at
//! "v:" ++ tag. Gas is the PVM's, the same in refine as it would be in accumulate.
#![no_std]
#![no_main]

extern crate alloc;

use ark_bn254::Fr;
use ark_ff::{BigInteger, PrimeField};
use jam_pvm_common::accumulate::{accumulate_items, set_storage};
use jam_pvm_common::jam_types::*;
use jam_pvm_common::{declare_service, Service};

declare_service!(Spike);
struct Spike;

fn fr(b: &[u8]) -> Fr { Fr::from_be_bytes_mod_order(b) }

fn run(p: &[u8]) -> Option<bool> {
    let kind = *p.first()?;
    let n = u16::from_le_bytes(p.get(1..3)?.try_into().ok()?) as usize;
    let expected = p.get(p.len().checked_sub(32)?..)?;
    let out = match kind {
        0 => {
            let mut x = Fr::from(1u64);
            for _ in 0..n {
                x = p2::hash(&[x, x]);
            }
            x
        }
        1 => {
            let leaf = fr(p.get(3..35)?);
            let index = u64::from_le_bytes(p.get(35..43)?.try_into().ok()?);
            let sibs: alloc::vec::Vec<Fr> = p.get(43..43 + 40 * 32)?.chunks_exact(32).map(fr).collect();
            let mut root = Fr::from(0u64);
            for _ in 0..n {
                root = p2::root_from_path(leaf, index, &sibs);
            }
            root
        }
        2 => {
            let mut start = u64::from_le_bytes(p.get(3..11)?.try_into().ok()?);
            let words = |at: usize, n: usize| -> Option<alloc::vec::Vec<Fr>> {
                Some(p.get(at..at + 32 * n)?.chunks_exact(32).map(fr).collect())
            };
            let frontier = words(11, 40)?;
            let zero = words(11 + 40 * 32, 40)?;
            let mut nodes = words(11 + 80 * 32, n)?;
            for level in 0..40 {
                if start & 1 == 1 {
                    nodes.insert(0, frontier[level]);
                    start -= 1;
                }
                nodes = nodes.chunks(2).map(|c| p2::hash(&[c[0], *c.get(1).unwrap_or(&zero[level])])).collect();
                start >>= 1;
            }
            *nodes.first()?
        }
        _ => return None,
    };
    Some(out.into_bigint().to_bytes_be()[..] == expected[..])
}

impl Service for Spike {
    fn refine(_core: CoreIndex, _item: usize, _service: ServiceId, payload: WorkPayload, _pkg: WorkPackageHash) -> WorkOutput {
        let p = payload.take();
        let tag = p.first().copied().unwrap_or(0);
        let ok = p.get(1..).and_then(run).unwrap_or(false);
        alloc::vec![tag, ok as u8].into()
    }

    fn accumulate(_slot: Slot, _service: ServiceId, _items: usize) -> Option<Hash> {
        for item in accumulate_items() {
            if let AccumulateItem::WorkItem(rec) = item {
                if let Ok(out) = rec.result {
                    if out.0.len() == 2 {
                        set_storage(&[b'v', b':', out.0[0]], &[out.0[1]]).ok();
                    }
                }
            }
        }
        None
    }
}
