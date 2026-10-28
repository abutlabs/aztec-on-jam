// aztec-on-jam#62 step 2: bb 6.0.0's CHONK verifier, unchanged, behind one C entry point,
// so a PVM service (or a native harness) can call it. The input carries everything a verifier
// needs, with no files:
//   [u32 proof_len][proof][u32 vk_len][vk][u32 n_bn254][n_bn254 x 64: G1 points][128: G2 point]
//   [u32 n_grumpkin][n_grumpkin x 64: Grumpkin points]    (u32 little-endian; points as bb's
//   CRS files store them)
// mode 0: the per-proof reduction (ChonkNativeVerifier::reduce_to_triple_ipa_opening);
// mode 1: the whole verification (verify), the TripleIPA included.
// Returns 1 valid, 0 invalid, -1 malformed input.
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

    auto proof = ChonkProof::from_field_elements(many_from_buffer<fr>(proof_bytes));
    auto vk = from_buffer<std::shared_ptr<MegaZKFlavor::VerificationKey>>(vk_bytes);
    ChonkNativeVerifier verifier(std::make_shared<ChonkNativeVerifier::VKAndHash>(vk));
    if (mode == 0) return verifier.reduce_to_triple_ipa_opening(proof).all_checks_passed ? 1 : 0;
    return verifier.verify(proof) ? 1 : 0;
}
