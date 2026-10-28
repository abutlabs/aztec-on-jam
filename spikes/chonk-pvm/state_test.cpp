// Native tests of state.cpp (aztec-on-jam#73). Aztec's own test vectors for its hashes, then whole
// transactions against small trees: the hints are made as Aztec's sequencer makes them (low leaves
// found and updated in descending order), and the end roots are checked against trees rebuilt from
// their final leaves alone. Then each of the circuit's checks, broken one at a time.
// With a file argument, a captured transaction instead ([1282 x F kernel public inputs][hints]
// [6 x F the end snapshots Aztec's prover computed], tools/aztec-local/capture-tx.mjs).
#include "state.cpp"
#include <algorithm>
#include <cstdio>
#include <functional>
#include <map>
#include <string>

namespace {
int failures = 0;
void check(bool ok, const std::string& what)
{
    std::printf("%s %s\n", ok ? "ok  " : "FAIL", what.c_str());
    failures += !ok;
}
fr hex(const char* s) { return fr(uint256_t(s)); }

/// A sparse Merkle tree: unset nodes are the empty subtree's root at their level.
struct Tree {
    size_t height;
    Hasher h;
    std::vector<fr> empty;
    std::map<std::pair<size_t, uint64_t>, fr> nodes;
    Tree(size_t height, Hasher h) : height(height), h(h)
    {
        empty.push_back(fr(0));
        for (size_t i = 0; i < height; ++i) empty.push_back(h(empty.back(), empty.back()));
    }
    fr node(size_t level, uint64_t i) const
    {
        auto it = nodes.find({ level, i });
        return it == nodes.end() ? empty[level] : it->second;
    }
    void set(uint64_t i, const fr& leaf)
    {
        nodes[{ 0, i }] = leaf;
        for (size_t l = 0; l < height; ++l, i >>= 1)
            nodes[{ l + 1, i >> 1 }] = h(node(l, i & ~1ULL), node(l, i | 1));
    }
    fr root() const { return node(height, 0); }
    std::vector<fr> path(uint64_t i, size_t from = 0) const
    {
        std::vector<fr> p;
        i >>= from;
        for (size_t l = from; l < height; ++l, i >>= 1) p.push_back(node(l, i ^ 1));
        return p;
    }
};

struct Bytes {
    std::vector<uint8_t> b;
    void f(const fr& v)
    {
        uint8_t o[32], *p = o;
        put(p, v);
        b.insert(b.end(), o, o + 32);
    }
    void u32(uint32_t v)
    {
        for (int i = 0; i < 4; ++i) b.push_back(uint8_t(v >> (8 * i)));
    }
    void fs(const std::vector<fr>& v)
    {
        for (auto& x : v) f(x);
    }
};

fr payer_slot(const fr& payer)
{
    return hash_with_separator(DOM_SEP__PUBLIC_LEAF_SLOT,
                               { fr(FEE_JUICE_ADDRESS),
                                 hash_with_separator(DOM_SEP__PUBLIC_STORAGE_MAP_SLOT, { fr(FEE_JUICE_BALANCES_SLOT), payer }) });
}

/// One transaction against a small state; `tamper` breaks the inputs before the call.
struct Case {
    std::vector<fr> pis = std::vector<fr>(PIS, fr(0));
    Tree note_hashes{ NOTE_HASH_TREE_HEIGHT, merkle_hash }, nullifiers{ NULLIFIER_TREE_HEIGHT, nullifier_hash },
        public_data{ PUBLIC_DATA_TREE_HEIGHT, public_data_hash };
    uint64_t note_next = 64, nullifier_next = 64;
    std::map<fr, std::pair<NullifierLeaf, uint64_t>, std::function<bool(const fr&, const fr&)>> leaves{ lt };
    PublicDataLeaf payer;
    uint64_t payer_index = 3;
    fr gas_da = fr(2), gas_l2 = fr(7);
    size_t omit = SIZE_MAX;  // a nullifier whose hint the sequencer leaves out, every other hint honest

    Case()
    {
        pis[MAX_FEES_AT] = fr(10); pis[MAX_FEES_AT + 1] = fr(20);
        pis[MAX_PRIORITY_AT] = fr(1); pis[MAX_PRIORITY_AT + 1] = fr(100);
        pis[GAS_USED_AT] = fr(1000); pis[GAS_USED_AT + 1] = fr(50000);
        pis[FEE_PAYER_AT] = fr(0xfee);
        note_hashes.set(0, fr(111)); note_hashes.set(1, fr(222));
        add_leaf({ fr(5), fr(20), fr(1) }, 0);
        add_leaf({ fr(20), fr(0), fr(0) }, 1);
        payer = { payer_slot(pis[FEE_PAYER_AT]), fr(10'000'000), fr(0), fr(0) };
        public_data.set(0, PublicDataLeaf{ fr(1), fr(9), payer.slot, fr(3) }.hash());
        public_data.set(payer_index, payer.hash());
    }
    void add_leaf(NullifierLeaf l, uint64_t i)
    {
        leaves[l.nullifier] = { l, i };
        nullifiers.set(i, l.hash());
    }

    /// The hints, as Aztec's sequencer makes them, then the call.
    int run(std::vector<fr> kernel_nullifiers, std::vector<fr> kernel_notes, std::vector<uint8_t>& out,
            const std::function<void(Bytes&, size_t)>& tamper = {})
    {
        for (size_t i = 0; i < kernel_notes.size(); ++i) pis[NOTE_HASHES_AT + i] = kernel_notes[i];
        for (size_t i = 0; i < kernel_nullifiers.size(); ++i) pis[NULLIFIERS_AT + i] = kernel_nullifiers[i];
        std::vector<size_t> order(kernel_nullifiers.size());
        for (size_t i = 0; i < order.size(); ++i) order[i] = i;
        order.erase(std::remove(order.begin(), order.end(), omit), order.end());
        std::sort(order.begin(), order.end(), [&](size_t a, size_t b) { return lt(kernel_nullifiers[b], kernel_nullifiers[a]); });
        Tree working = nullifiers;
        auto working_leaves = leaves;
        Bytes in, entries;
        for (size_t i : order) {
            fr v = kernel_nullifiers[i];
            auto low = std::prev(working_leaves.lower_bound(v));
            auto [leaf, at] = low->second;
            entries.f(v); entries.u32(uint32_t(i));
            entries.fs({ leaf.nullifier, leaf.next_nullifier, leaf.next_index });
            entries.f(fr(at)); entries.fs(working.path(at));
            low->second.first = { leaf.nullifier, v, fr(nullifier_next + i) };
            working.set(at, low->second.first.hash());
        }
        in.fs({ note_hashes.root(), fr(note_next), nullifiers.root(), fr(nullifier_next), public_data.root(), fr(64) });
        in.fs({ gas_da, gas_l2 });
        in.fs({ payer.slot, payer.value, payer.next_slot, payer.next_index });
        in.f(fr(payer_index)); in.fs(public_data.path(payer_index));
        in.fs(note_hashes.path(note_next, SUBTREE_HEIGHT));
        in.fs(working.path(nullifier_next, SUBTREE_HEIGHT));
        in.u32(uint32_t(order.size()));
        size_t entries_at = in.b.size();
        in.b.insert(in.b.end(), entries.b.begin(), entries.b.end());
        if (tamper) tamper(in, entries_at);
        out.assign(7 * 32, 0);
        return aztec_tx_transition(pis.data(), in.b.data(), in.b.size(), out.data());
    }

    /// The end state rebuilt from its final leaves alone, not from the hints.
    std::vector<fr> expected(std::vector<fr> kernel_nullifiers, std::vector<fr> kernel_notes, const fr& fee)
    {
        Tree n = note_hashes, x(NULLIFIER_TREE_HEIGHT, nullifier_hash), p = public_data;
        for (size_t i = 0; i < kernel_notes.size(); ++i) n.set(note_next + i, kernel_notes[i]);
        auto all = leaves;
        for (size_t i = 0; i < kernel_nullifiers.size(); ++i) all[kernel_nullifiers[i]] = { {}, nullifier_next + i };
        for (auto it = all.begin(); it != all.end(); ++it) {
            auto next = std::next(it);
            NullifierLeaf l{ it->first, next == all.end() ? fr(0) : next->first, next == all.end() ? fr(0) : fr(next->second.second) };
            x.set(it->second.second, l.hash());
        }
        p.set(payer_index, PublicDataLeaf{ payer.slot, payer.value - fee, payer.next_slot, payer.next_index }.hash());
        return { n.root(), fr(note_next + 64), x.root(), fr(nullifier_next + 64), p.root(), fr(64), fee };
    }
};

std::vector<fr> fields(const std::vector<uint8_t>& out)
{
    Reader r{ out.data(), out.size() };
    return r.fs(out.size() / 32);
}

void flip(Bytes& in, size_t at) { in.b[at + 31] ^= 1; }

void transactions()
{
    std::vector<fr> nul{ fr(25), fr(7), fr(12) }, notes{ fr(333), fr(444) };
    // fee: DA 2 + min(1, 10-2) = 3, L2 7 + min(100, 20-7) = 20 → 1000*3 + 50000*20
    fr fee(1000 * 3 + 50000 * 20);
    std::vector<uint8_t> out;
    {
        Case c;
        int v = c.run(nul, notes, out);
        check(v == 1 && fields(out) == c.expected(nul, notes, fee),
              "three nullifiers, two note hashes, the fee: end roots = trees rebuilt from their leaves");
    }
    {
        Case c;
        std::vector<fr> one{ fr(21) };
        int v = c.run(one, {}, out);
        check(v == 1 && fields(out) == c.expected(one, {}, fee), "one nullifier above the largest (low leaf points to infinity)");
    }
    struct Bad {
        const char* what;
        std::function<void(Case&)> setup;
        std::function<void(Bytes&, size_t)> tamper;
        std::vector<fr> nul;
    };
    const size_t E = 32 + 4 + 96 + 32;  // an entry's offset to its first sibling
    std::vector<Bad> bad{
        { "a nullifier already in the tree", {}, {}, { fr(20) } },
        { "a nullifier twice", {}, {}, { fr(9), fr(9) } },
        { "the start note hash root", {}, [](Bytes& in, size_t) { flip(in, 0); }, nul },
        { "the start nullifier root", {}, [](Bytes& in, size_t) { flip(in, 64); }, nul },
        { "the start public data root", {}, [](Bytes& in, size_t) { flip(in, 128); }, nul },
        { "a low leaf's sibling", {}, [&](Bytes& in, size_t at) { flip(in, at + E); }, nul },
        { "a low leaf's preimage", {}, [](Bytes& in, size_t at) { flip(in, at + 36 + 32); }, nul },
        { "a sorted nullifier's index", {}, [](Bytes& in, size_t at) { in.b[at + 32] ^= 1; }, nul },
        { "an entry left out", {}, [](Bytes& in, size_t at) { in.b[at - 4] = 2; in.b.resize(in.b.size() - (32 + 4 + 96 + 32 + 42 * 32)); }, nul },
        { "the fee payer's balance leaf", {}, [](Bytes& in, size_t) { flip(in, 8 * 32 + 32); }, nul },
        { "a balance below the fee", [](Case& c) { c.payer.value = fr(5); c.public_data.set(c.payer_index, c.payer.hash()); }, {}, nul },
        { "another account's balance leaf", [](Case& c) { c.pis[FEE_PAYER_AT] = fr(0xbad); }, {}, nul },
        { "gas fees above the maximum", [](Case& c) { c.gas_l2 = fr(21); }, {}, nul },
        { "a full note hash subtree slot", [](Case& c) { c.note_hashes.set(70, fr(1)); }, {}, nul },
        { "a nullifier left out of honest hints", [](Case& c) { c.omit = 1; }, {}, nul },
        { "a trailing octet", {}, [](Bytes& in, size_t) { in.b.push_back(0); }, nul },
    };
    for (auto& t : bad) {
        Case c;
        if (t.setup) t.setup(c);
        int v = c.run(t.nul, notes, out, t.tamper);
        check(v != 1, std::string("refused: ") + t.what + " (" + std::to_string(v) + ")");
    }
}

void vectors()
{
    std::vector<fr> sixteen;
    for (int i = 1; i <= 16; ++i) sixteen.push_back(fr(i));
    check(tree_root(sixteen, merkle_hash) == hex("0x2bc86dba04dfdd6352c3b1c66b2300445964e2888aa52fdb023d2e645a3d3399"),
          "merkle root of 1..16 (types merkle_tree/root.nr)");
    check(tree_root(std::vector<fr>(16, fr(0)), merkle_hash) == hex("0x1e20ad4181460cbfdc74ca773502c59b890f184efe300ebad895956d318422da"),
          "merkle root of 16 zeros");
    check(empty_root(1, merkle_hash) == hex("0x19f1a0c09db4cd026f686e9c8fb45501a9fefb4eb1b4c6c328a51343a0094eeb"), "empty root, height 1");
    check(empty_root(6, merkle_hash) == hex("0x119f56a2e8423a7feaab49b9b5dcbadec0648dfa4096b61b6774ea33ae29dc7f"), "empty root, height 6");
    check(empty_root(10, merkle_hash) == hex("0x0d04c63f36bd168215c9b09a227c7e8d3ad48e2f11b8202fd07c524bd30ee88f"), "empty root, height 10");
    check(PublicDataLeaf{ fr(123), fr(45), fr(67), fr(890) }.hash() ==
              hex("0x2efdfcfc865cbb7543183fae69374ee5106dde9741545afd2fbf12868b550614"),
          "public data leaf hash (data/public_data_tree_leaf_preimage.nr)");
}

int captured(const char* path)
{
    FILE* f = std::fopen(path, "rb");
    std::vector<uint8_t> b;
    for (int c; (c = std::fgetc(f)) != EOF;) b.push_back(uint8_t(c));
    std::fclose(f);
    if (b.size() < (PIS + 6) * 32) return 2;
    Reader r{ b.data(), PIS * 32 };
    auto pis = r.fs(PIS);
    size_t hints = b.size() - (PIS + 6) * 32;
    std::vector<uint8_t> out(7 * 32);
    int v = aztec_tx_transition(pis.data(), b.data() + PIS * 32, hints, out.data());
    Reader e{ b.data() + PIS * 32 + hints, 6 * 32 };
    auto want = e.fs(6), got = fields(out);
    got.resize(6);
    check(v == 1, std::string(path) + ": applied (" + std::to_string(v) + ")");
    check(got == want, "end snapshots = Aztec's prover's");
    uint256_t fee(fields(out)[6]);
    std::printf("fee %llu (high words %s)\n", (unsigned long long)fee.data[0], (fee >> 64) == 0 ? "zero" : "NOT zero");
    return failures ? 1 : 0;
}
} // namespace

int main(int argc, char** argv)
{
    if (argc > 1) return captured(argv[1]);
    vectors();
    transactions();
    std::printf("%s\n", failures ? "FAILED" : "all passed");
    return failures ? 1 : 0;
}
