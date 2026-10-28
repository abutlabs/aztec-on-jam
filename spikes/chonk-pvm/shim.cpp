// aztec-on-jam#62 step 2: bb 6.0.0's CHONK verifier, unchanged, behind one C entry point,
// so a PVM service (or a native harness) can call it. The input carries everything a verifier
// needs, with no files:
//   [u32 proof_len][proof][u32 vk_len][vk][u32 n_bn254][n_bn254 x 64: G1 points][128: G2 point]
//   [u32 n_grumpkin][n_grumpkin x 64: Grumpkin points]    (u32 little-endian; points as bb's
//   CRS files store them)
// mode 0: the per-proof reduction (ChonkNativeVerifier::reduce_to_triple_ipa_opening);
// mode 1: the whole verification (verify), the TripleIPA included.
// Returns 1 valid, 0 invalid, -1 malformed input.
//
// chonk_pvm_verify_crs takes the proof and vk as [u32 proof_len][proof][u32 vk_len][vk] and the
// CRS apart, already in bb's in-memory form (crs-raw.py writes it; a service keeps it as a
// preimage): [u32 n_bn254][n_bn254 x sizeof(g1::affine_element)][sizeof(g2::affine_element)]
// [u32 n_grumpkin][n_grumpkin x sizeof(Grumpkin::AffineElement)]. The points are copied, not
// parsed: parsing 2^15 Grumpkin points costs 261M gas in refine (RESULTS.md).
//
// chonk_pvm_verify_batch checks several proofs the way bb's ChonkBatchVerifier does, without its
// threads: each proof reduced to its deferred TripleIPA opening, then every passing proof's
// accumulator checked together by one batch IPA (ECCVMVerifier::batch_verify_accumulators), whose
// one 2^15 Grumpkin MSM is shared. Input: [u32 n] then n x [u32 proof_len][proof][u32 vk_len][vk];
// the CRS as for chonk_pvm_verify_crs. verdicts[i]: 1 valid, 0 its reduction failed, 4 its
// reduction passed but the batch IPA failed (one of the batch is bad: check them apart). Returns
// n, or -1 for a malformed input.
//
// chonk_pvm_verify_tx verifies one private transaction's proof, then applies its effect to Aztec's
// trees (state.cpp) with the kernel's public inputs read from that proof, so what is inserted is
// what was proven. Input: [u32 proof_len][proof][u32 vk_len][vk][state.cpp's hints]; the CRS as
// for chonk_pvm_verify_crs. Writes state.cpp's output (end snapshots, fee) to `out`. Returns 1
// applied, 0 the proof invalid, 5 the proof valid but the transition refused, -1 malformed.
#include "barretenberg/chonk/chonk_verifier.hpp"
#include "barretenberg/common/log.hpp"
#include "barretenberg/common/serialize.hpp"
#include "barretenberg/srs/global_crs.hpp"
#include <cstdint>
#include <cstring>
#include <vector>

namespace {
struct Reader {
    const uint8_t* p;
    size_t left;
    bool ok = true;
    uint32_t u32()
    {
        if (left < 4) { ok = false; return 0; }
        uint32_t v = uint32_t(p[0]) | uint32_t(p[1]) << 8 | uint32_t(p[2]) << 16 | uint32_t(p[3]) << 24;
        p += 4; left -= 4;
        return v;
    }
    std::vector<uint8_t> bytes(size_t n)
    {
        if (left < n) { ok = false; return {}; }
        std::vector<uint8_t> v(p, p + n);
        p += n; left -= n;
        return v;
    }
};
} // namespace

namespace {
int verify_with(const std::vector<uint8_t>& proof_bytes, const std::vector<uint8_t>& vk_bytes, int mode)
{
    using namespace bb;
    auto proof = ChonkProof::from_field_elements(many_from_buffer<fr>(proof_bytes));
    auto vk = from_buffer<std::shared_ptr<MegaZKFlavor::VerificationKey>>(vk_bytes);
    ChonkNativeVerifier verifier(std::make_shared<ChonkNativeVerifier::VKAndHash>(vk));
    if (mode == 0) return verifier.reduce_to_triple_ipa_opening(proof).all_checks_passed ? 1 : 0;
    return verifier.verify(proof) ? 1 : 0;
}

template <typename T> bool copy_points(Reader& r, std::vector<T>& out, uint32_t n)
{
    auto raw = r.bytes(size_t(n) * sizeof(T));
    if (!r.ok) return false;
    out.resize(n);
    std::memcpy(out.data(), raw.data(), raw.size());
    return true;
}
} // namespace

namespace {
bool init_crs(const uint8_t* crs, size_t crs_len)
{
    using namespace bb;
    Reader c{ crs, crs_len };
    std::vector<g1::affine_element> bn_points;
    std::vector<g2::affine_element> g2_point;
    std::vector<curve::Grumpkin::AffineElement> gr_points;
    uint32_t n_bn = c.u32();
    if (!c.ok || n_bn == 0 || !copy_points(c, bn_points, n_bn) || !copy_points(c, g2_point, 1)) return false;
    uint32_t n_gr = c.u32();
    if (!c.ok || !copy_points(c, gr_points, n_gr)) return false;
    srs::init_bn254_mem_crs_factory(bn_points, g2_point[0]);
    srs::init_grumpkin_mem_crs_factory(gr_points);
    return true;
}
} // namespace

extern "C" int chonk_pvm_verify_batch(const uint8_t* in, size_t len, const uint8_t* crs, size_t crs_len, uint8_t* verdicts, size_t max)
{
    using namespace bb;
    bb_log_level = LogLevel::SILENT;
    Reader r{ in, len };
    uint32_t n = r.u32();
    if (!r.ok || n == 0 || n > max || !init_crs(crs, crs_len)) return -1;
    std::vector<ECCVMVerifier::DeferredTripleIpaOpening::Accumulator> accumulators;
    std::vector<uint32_t> passed;
    for (uint32_t i = 0; i < n; ++i) {
        auto proof_bytes = r.bytes(r.u32());
        auto vk_bytes = r.bytes(r.u32());
        if (!r.ok) return -1;
        auto proof = ChonkProof::from_field_elements(many_from_buffer<fr>(proof_bytes));
        auto vk = from_buffer<std::shared_ptr<MegaZKFlavor::VerificationKey>>(vk_bytes);
        ChonkNativeVerifier verifier(std::make_shared<ChonkNativeVerifier::VKAndHash>(vk));
        auto reduced = verifier.reduce_to_triple_ipa_opening(proof);
        verdicts[i] = 0;
        if (reduced.all_checks_passed) {
            accumulators.push_back(reduced.triple_ipa_opening.reduce_to_accumulator());
            passed.push_back(i);
        }
    }
    if (r.left != 0) return -1;
    bool batch_ok = accumulators.empty() || ECCVMVerifier::batch_verify_accumulators(accumulators);
    for (uint32_t i : passed) verdicts[i] = batch_ok ? 1 : 4;
    return int(n);
}

extern "C" int aztec_tx_transition(const bb::fr* pis, const uint8_t* in, size_t len, uint8_t* out);

extern "C" int chonk_pvm_verify_tx(const uint8_t* in, size_t len, const uint8_t* crs, size_t crs_len, uint8_t* out)
{
    using namespace bb;
    // the kernel's public inputs (PrivateToRollupKernelCircuitPublicInputs), first in the proof
    constexpr size_t KERNEL_PIS = 1282;
    bb_log_level = LogLevel::SILENT;
    Reader r{ in, len };
    auto proof_bytes = r.bytes(r.u32());
    auto vk_bytes = r.bytes(r.u32());
    if (!r.ok || proof_bytes.size() < KERNEL_PIS * 32 || !init_crs(crs, crs_len)) return -1;
    if (verify_with(proof_bytes, vk_bytes, 1) != 1) return 0;
    auto pis = many_from_buffer<fr>(proof_bytes);
    int applied = aztec_tx_transition(pis.data(), r.p, r.left, out);
    return applied == 1 ? 1 : applied == 0 ? 5 : -1;
}

extern "C" int chonk_pvm_verify_crs(const uint8_t* in, size_t len, const uint8_t* crs, size_t crs_len, int mode)
{
    using namespace bb;
    bb_log_level = LogLevel::SILENT;
    Reader r{ in, len };
    auto proof_bytes = r.bytes(r.u32());
    auto vk_bytes = r.bytes(r.u32());
    if (!r.ok || !init_crs(crs, crs_len)) return -1;
    return verify_with(proof_bytes, vk_bytes, mode);
}

extern "C" int chonk_pvm_verify(const uint8_t* in, size_t len, int mode)
{
    using namespace bb;
    // A refine has nowhere to print; bb's embedders set its log level the same way.
    bb_log_level = LogLevel::SILENT;
    Reader r{ in, len };
    auto proof_bytes = r.bytes(r.u32());
    auto vk_bytes = r.bytes(r.u32());
    uint32_t n_bn = r.u32();
    auto bn = r.bytes(size_t(n_bn) * 64);
    auto g2 = r.bytes(128);
    uint32_t n_gr = r.u32();
    auto gr = r.bytes(size_t(n_gr) * 64);
    if (!r.ok || n_bn == 0) return -1;

    std::vector<g1::affine_element> bn_points(n_bn);
    for (size_t i = 0; i < n_bn; ++i) bn_points[i] = from_buffer<g1::affine_element>(bn, i * 64);
    srs::init_bn254_mem_crs_factory(bn_points, from_buffer<g2::affine_element>(g2.data()));
    std::vector<curve::Grumpkin::AffineElement> gr_points(n_gr);
    for (size_t i = 0; i < n_gr; ++i) gr_points[i] = from_buffer<curve::Grumpkin::AffineElement>(gr, i * 64);
    srs::init_grumpkin_mem_crs_factory(gr_points);
    return verify_with(proof_bytes, vk_bytes, mode);
}
