// aztec-on-jam#73: a private transaction's effect on Aztec's trees, as Aztec 6.0.0's private tx
// base rollup computes it (noir-protocol-circuits rollup-lib tx_base: build_tree_snapshots,
// update_public_data_tree_leaf, fees), ported line for line with bb's Poseidon2. Refine runs it
// on the public inputs of the client proof it has just verified, so what goes into the trees is
// exactly what the proof proves; accumulate then checks the start snapshots against the ones it
// holds and stores the end ones.
//
// aztec_tx_transition(pis, in, len, out): `pis` the kernel's 1,282 public inputs
// (PrivateToRollupKernelCircuitPublicInputs, the first fields of the proof), `in` the hints, each
// F a 32-octet big-endian field:
//   start snapshots: note hash (root F, next index F), nullifier (F, F), public data (F, F)
//   gas fees: fee per DA gas F, fee per L2 gas F
//   the fee payer's balance leaf: slot F, value F, next slot F, next index F
//   its witness: leaf index F, 40 x F
//   the note hash subtree's sibling path: 36 x F
//   the nullifier subtree's sibling path: 36 x F
//   [u32 k] k x { sorted nullifier F, [u32 its index in the kernel's nullifiers],
//                 low leaf: nullifier F, next nullifier F, next index F, its witness: F, 42 x F }
// The k entries are the non-empty ones of Aztec's sorted hints, in their order (the circuit skips
// the empty ones). Writes the end snapshots (6 x F) and the transaction fee (F) to `out`.
// Returns 1 applied, 0 a check of Aztec's circuit failed, -1 malformed.
#include "barretenberg/crypto/poseidon2/poseidon2.hpp"
#include "barretenberg/ecc/curves/bn254/fr.hpp"
#include <array>
#include <cstdint>
#include <cstring>
#include <vector>

namespace {
using bb::fr;
using Poseidon2 = bb::crypto::Poseidon2<bb::crypto::Poseidon2Bn254ScalarFieldParams>;

constexpr size_t NOTE_HASH_TREE_HEIGHT = 42, NULLIFIER_TREE_HEIGHT = 42, PUBLIC_DATA_TREE_HEIGHT = 40;
constexpr size_t SUBTREE_HEIGHT = 6, SUBTREE_WIDTH = 64;  // note hashes and nullifiers alike
constexpr size_t PIS = 1282, NOTE_HASHES_AT = 35, NULLIFIERS_AT = 99;
constexpr size_t MAX_FEES_AT = 29, MAX_PRIORITY_AT = 31, GAS_USED_AT = 1278, FEE_PAYER_AT = 1280;
constexpr uint64_t DOM_SEP__MERKLE_HASH = 2982624097, DOM_SEP__NULLIFIER_MERKLE = 1157584160,
                   DOM_SEP__PUBLIC_DATA_MERKLE = 3756303423, DOM_SEP__PUBLIC_STORAGE_MAP_SLOT = 4015149901,
                   DOM_SEP__PUBLIC_LEAF_SLOT = 1247650290, FEE_JUICE_ADDRESS = 3, FEE_JUICE_BALANCES_SLOT = 1;

fr hash_with_separator(uint64_t sep, std::vector<fr> in)
{
    in.insert(in.begin(), fr(sep));
    return Poseidon2::hash(in);
}

struct Hasher {
    uint64_t sep;
    fr operator()(const fr& l, const fr& r) const { return Poseidon2::hash({ fr(sep), l, r }); }
};
const Hasher merkle_hash{ DOM_SEP__MERKLE_HASH }, nullifier_hash{ DOM_SEP__NULLIFIER_MERKLE },
    public_data_hash{ DOM_SEP__PUBLIC_DATA_MERKLE };

bool lt(const fr& a, const fr& b) { return uint256_t(a) < uint256_t(b); }

struct Snapshot {
    fr root, next;
};
struct NullifierLeaf {
    fr nullifier, next_nullifier, next_index;
    bool empty() const { return nullifier.is_zero() && next_nullifier.is_zero() && next_index.is_zero(); }
    fr hash() const { return empty() ? fr(0) : Poseidon2::hash({ nullifier, next_nullifier, next_index }); }
};
struct PublicDataLeaf {
    fr slot, value, next_slot, next_index;
    fr hash() const { return Poseidon2::hash({ slot, value, next_slot, next_index }); }
};
struct Witness {
    fr index;
    std::vector<fr> path;
};

fr root_from_path(fr node, const fr& index, const std::vector<fr>& path, const Hasher& h)
{
    uint256_t bits(index);
    for (size_t i = 0; i < path.size(); ++i) node = bits.get_bit(i) ? h(path[i], node) : h(node, path[i]);
    return node;
}

fr tree_root(std::vector<fr> level, const Hasher& h)
{
    while (level.size() > 1) {
        std::vector<fr> up(level.size() / 2);
        for (size_t i = 0; i < up.size(); ++i) up[i] = h(level[2 * i], level[2 * i + 1]);
        level = std::move(up);
    }
    return level[0];
}

fr empty_root(size_t height, const Hasher& h)
{
    fr node(0);
    for (size_t i = 0; i < height; ++i) node = h(node, node);
    return node;
}

// append_only_tree::insert_subtree_root_to_snapshot_with_hasher
bool insert_subtree(Snapshot& s, const std::vector<fr>& path, const fr& subtree_root, const Hasher& h)
{
    uint256_t next(s.next);
    if (next >> 64 != 0) return false;
    fr at(next >> SUBTREE_HEIGHT);
    if (root_from_path(empty_root(SUBTREE_HEIGHT, h), at, path, h) != s.root) return false;
    s.root = root_from_path(subtree_root, at, path, h);
    s.next = s.next + fr(SUBTREE_WIDTH);
    return true;
}

struct Reader {
    const uint8_t* p;
    size_t left;
    bool ok = true;
    fr f()
    {
        if (left < 32) { ok = false; return fr(0); }
        uint256_t v(0);
        for (size_t i = 0; i < 32; ++i) v = (v << 8) + uint256_t(p[i]);
        p += 32; left -= 32;
        if (v >= fr::modulus) ok = false;
        return fr(v);
    }
    uint32_t u32()
    {
        if (left < 4) { ok = false; return 0; }
        uint32_t v = uint32_t(p[0]) | uint32_t(p[1]) << 8 | uint32_t(p[2]) << 16 | uint32_t(p[3]) << 24;
        p += 4; left -= 4;
        return v;
    }
    std::vector<fr> fs(size_t n)
    {
        std::vector<fr> v(n);
        for (auto& x : v) x = f();
        return v;
    }
    Witness witness(size_t height)
    {
        Witness w;
        w.index = f();
        w.path = fs(height);
        return w;
    }
};

void put(uint8_t*& out, const fr& v)
{
    uint256_t x(v);
    for (int i = 31; i >= 0; --i) { out[i] = uint8_t(x.data[0] & 0xff); x >>= 8; }
    out += 32;
}

// u128 as the circuit holds GasFees: a field above 2^128 is not one
bool u128(const fr& v) { return uint256_t(v) >> 128 == 0; }
} // namespace

extern "C" int aztec_tx_transition(const fr* pis, const uint8_t* in, size_t len, uint8_t* out)
{
    Reader r{ in, len };
    Snapshot note_hash{ r.f(), r.f() }, nullifier{ r.f(), r.f() }, public_data{ r.f(), r.f() };
    fr gas_fee_da = r.f(), gas_fee_l2 = r.f();
    PublicDataLeaf payer{ r.f(), r.f(), r.f(), r.f() };
    Witness payer_witness = r.witness(PUBLIC_DATA_TREE_HEIGHT);
    auto note_hash_path = r.fs(NOTE_HASH_TREE_HEIGHT - SUBTREE_HEIGHT);
    auto nullifier_path = r.fs(NULLIFIER_TREE_HEIGHT - SUBTREE_HEIGHT);
    uint32_t k = r.u32();
    if (!r.ok || k > SUBTREE_WIDTH) return -1;
    struct Entry {
        fr value;
        uint32_t index;
        NullifierLeaf low;
        Witness witness;
    };
    std::vector<Entry> sorted(k);
    for (auto& e : sorted) {
        e.value = r.f();
        e.index = r.u32();
        e.low = { r.f(), r.f(), r.f() };
        e.witness = r.witness(NULLIFIER_TREE_HEIGHT);
    }
    if (!r.ok || r.left != 0) return -1;

    // fees::compute_transaction_fee, in the circuit's u32 gas and u128 fees
    fr max_da = pis[MAX_FEES_AT], max_l2 = pis[MAX_FEES_AT + 1];
    fr prio_da = pis[MAX_PRIORITY_AT], prio_l2 = pis[MAX_PRIORITY_AT + 1];
    if (!u128(gas_fee_da) || !u128(gas_fee_l2) || !u128(max_da) || !u128(max_l2) || !u128(prio_da) || !u128(prio_l2))
        return 0;
    if (lt(max_da, gas_fee_da) || lt(max_l2, gas_fee_l2)) return 0;  // u128 subtraction underflows
    auto min = [](const fr& a, const fr& b) { return lt(a, b) ? a : b; };
    fr eff_da = gas_fee_da + min(prio_da, max_da - gas_fee_da), eff_l2 = gas_fee_l2 + min(prio_l2, max_l2 - gas_fee_l2);
    fr fee = pis[GAS_USED_AT] * eff_da + pis[GAS_USED_AT + 1] * eff_l2;

    // private_tx_base_inputs_validator::validate_fee_payer_balance
    fr map_slot = hash_with_separator(DOM_SEP__PUBLIC_STORAGE_MAP_SLOT, { fr(FEE_JUICE_BALANCES_SLOT), pis[FEE_PAYER_AT] });
    if (payer.slot != hash_with_separator(DOM_SEP__PUBLIC_LEAF_SLOT, { fr(FEE_JUICE_ADDRESS), map_slot })) return 0;
    if (lt(payer.value, fee)) return 0;

    // note hashes: their 64-leaf subtree into the snapshot
    std::vector<fr> note_hashes(pis + NOTE_HASHES_AT, pis + NOTE_HASHES_AT + SUBTREE_WIDTH);
    if (!insert_subtree(note_hash, note_hash_path, tree_root(note_hashes, merkle_hash), merkle_hash)) return 0;

    // nullifiers: indexed_tree::batch_insert_no_update. The k entries with the kernel's empty
    // nullifiers make the circuit's permutation: each index once, and every non-empty one covered.
    std::array<bool, SUBTREE_WIDTH> seen{};
    size_t non_empty = 0;
    for (size_t i = 0; i < SUBTREE_WIDTH; ++i) non_empty += !pis[NULLIFIERS_AT + i].is_zero();
    if (non_empty != k) return 0;
    std::vector<fr> appended(SUBTREE_WIDTH, fr(0));
    fr root = nullifier.root;
    for (auto& e : sorted) {
        if (e.index >= SUBTREE_WIDTH || seen[e.index] || e.value.is_zero() || pis[NULLIFIERS_AT + e.index] != e.value)
            return 0;
        seen[e.index] = true;
        if (e.low.empty()) return 0;
        bool to_infinity = e.low.next_nullifier.is_zero() && e.low.next_index.is_zero();
        if (!lt(e.low.nullifier, e.value) || !(lt(e.value, e.low.next_nullifier) || to_infinity)) return 0;
        if (root_from_path(e.low.hash(), e.witness.index, e.witness.path, nullifier_hash) != root) return 0;
        appended[e.index] = NullifierLeaf{ e.value, e.low.next_nullifier, e.low.next_index }.hash();
        NullifierLeaf updated{ e.low.nullifier, e.value, nullifier.next + fr(e.index) };
        root = root_from_path(updated.hash(), e.witness.index, e.witness.path, nullifier_hash);
    }
    nullifier.root = root;
    if (!insert_subtree(nullifier, nullifier_path, tree_root(appended, nullifier_hash), nullifier_hash)) return 0;

    // update_public_data_tree_leaf: the fee payer's new balance
    if (root_from_path(payer.hash(), payer_witness.index, payer_witness.path, public_data_hash) != public_data.root)
        return 0;
    PublicDataLeaf paid{ payer.slot, payer.value - fee, payer.next_slot, payer.next_index };
    public_data.root = root_from_path(paid.hash(), payer_witness.index, payer_witness.path, public_data_hash);

    for (auto* s : { &note_hash, &nullifier, &public_data }) { put(out, s->root); put(out, s->next); }
    put(out, fee);
    return 1;
}
