// jam-privacy-service#62: bb's Grumpkin MSM through the proposed host call 105
// (grumpkin_g1_msm), where the net has it. bb is not changed: the service links with
// --wrap for bb's two Grumpkin MSM entry points (service/build.rs), so every caller outside
// scalar_multiplication.cpp (the IPA, the ECCVM verifier, batch_mul) reaches these functions,
// and __real_ is bb's own MSM.
//
// The call's input is n items of (x ‖ y, scalar), 32 octets each, big-endian, (0, 0) for
// infinity; the scalars pair with points[start_index + i], as in bb's MSM. The service's
// chonk_gk_msm (crates/curve-calls' probe) answers 0 with the sum written, 1 when the call is
// absent (bb's MSM then runs as PVM code), and anything else for a malformed input or a host
// that answered WHAT after answering: that stops the refine (fail closed), as the inputs here
// come from bb's own checked proof and CRS.
#include "barretenberg/ecc/curves/grumpkin/grumpkin.hpp"
#include "barretenberg/ecc/scalar_multiplication/scalar_multiplication.hpp"
#include <cstdint>
#include <optional>
#include <vector>

using G = bb::curve::Grumpkin;
using Scalars = bb::PolynomialSpan<const G::ScalarField>;
using Points = std::span<const G::AffineElement>;

extern "C" int chonk_gk_msm(const uint8_t* input, size_t len, uint8_t* out);
extern "C" [[noreturn]] void chonk_trap(const char* what);

#define GK_PIPPENGER "_ZN2bb21scalar_multiplication9pippengerINS_5curve8GrumpkinEEENT_7ElementENS_14PolynomialSpanIKNS4_11ScalarFieldEEENSt3__14spanIKNS4_13AffineElementELm18446744073709551615EEEbm"
#define GK_PIPPENGER_UNSAFE "_ZN2bb21scalar_multiplication16pippenger_unsafeINS_5curve8GrumpkinEEENT_7ElementENS_14PolynomialSpanIKNS4_11ScalarFieldEEENSt3__14spanIKNS4_13AffineElementELm18446744073709551615EEEm"

G::Element real_pippenger(Scalars, Points, bool, size_t) noexcept __asm__("__real_" GK_PIPPENGER);
G::Element real_pippenger_unsafe(Scalars, Points, size_t) noexcept __asm__("__real_" GK_PIPPENGER_UNSAFE);

namespace {

uint64_t calls_made = 0;

template <typename F> void put(std::vector<uint8_t>& o, const F& f)
{
    const bb::numeric::uint256_t v(f);
    for (int limb = 3; limb >= 0; --limb) {
        for (int b = 7; b >= 0; --b) {
            o.push_back(uint8_t(v.data[limb] >> (8 * b)));
        }
    }
}

G::BaseField get(const uint8_t* p)
{
    bb::numeric::uint256_t v;
    for (int limb = 0; limb < 4; ++limb) {
        uint64_t w = 0;
        for (int b = 0; b < 8; ++b) {
            w = (w << 8) | p[8 * (3 - limb) + b];
        }
        v.data[limb] = w;
    }
    return G::BaseField(v);
}

// the sum through call 105, or nothing if the call is absent
std::optional<G::Element> via_call(Scalars scalars, Points points)
{
    const size_t n = scalars.size();
    if (n == 0) {
        return G::Element::zero();
    }
    std::vector<uint8_t> in;
    in.reserve(96 * n);
    for (size_t i = 0; i < n; ++i) {
        const auto& p = points[scalars.start_index + i];
        if (p.is_point_at_infinity()) {
            in.insert(in.end(), 64, 0);
        } else {
            put(in, p.x);
            put(in, p.y);
        }
        put(in, scalars.span[i]);
    }
    uint8_t out[64];
    switch (chonk_gk_msm(in.data(), in.size(), out)) {
    case 0:
        break;
    case 1:
        return std::nullopt;
    default:
        chonk_trap("call 105 refused bb's Grumpkin MSM");
    }
    ++calls_made;
    bool zero = true;
    for (uint8_t b : out) {
        zero = zero && b == 0;
    }
    if (zero) {
        return G::Element::zero();
    }
    return G::Element(G::AffineElement(get(out), get(out + 32)));
}

} // namespace

// how many MSMs went to the call, for the service's output
extern "C" uint64_t chonk_gk_calls() { return calls_made; }

G::Element wrap_pippenger(Scalars scalars, Points points, bool edge, size_t dedup) noexcept __asm__("__wrap_" GK_PIPPENGER);
G::Element wrap_pippenger(Scalars scalars, Points points, bool edge, size_t dedup) noexcept
{
    if (auto r = via_call(scalars, points)) {
        return *r;
    }
    return real_pippenger(scalars, points, edge, dedup);
}

G::Element wrap_pippenger_unsafe(Scalars scalars, Points points, size_t dedup) noexcept __asm__("__wrap_" GK_PIPPENGER_UNSAFE);
G::Element wrap_pippenger_unsafe(Scalars scalars, Points points, size_t dedup) noexcept
{
    if (auto r = via_call(scalars, points)) {
        return *r;
    }
    return real_pippenger_unsafe(scalars, points, dedup);
}
