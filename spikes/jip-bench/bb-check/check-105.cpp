// Checks call 105's vectors (lasair#130) with Barretenberg's own Grumpkin: decode as the JIP
// says (x ‖ y big-endian below r, (0, 0) infinity, on the curve; scalars mod p), sum, encode.
// Input lines: "<OK|HUH> <input hex> <output hex or ->". Prints a verdict per vector.
#include "barretenberg/ecc/curves/grumpkin/grumpkin.hpp"
#include <cstdio>
#include <iostream>
#include <string>
#include <vector>
using namespace bb;
static std::vector<uint8_t> unhex(const std::string& h)
{
    std::vector<uint8_t> b(h.size() / 2);
    for (size_t i = 0; i < b.size(); i++) b[i] = (uint8_t)std::stoi(h.substr(2 * i, 2), nullptr, 16);
    return b;
}
static uint256_t be(const uint8_t* p)
{
    uint256_t v = 0;
    for (int i = 0; i < 32; i++) v = (v << 8) + uint256_t(p[i]);
    return v;
}
static void put(uint8_t* o, uint256_t v)
{
    for (int i = 31; i >= 0; i--) { o[i] = (uint8_t)(v.data[0] & 0xff); v >>= 8; }
}
// nullopt-like: returns false for HUH
static bool msm(const std::vector<uint8_t>& in, std::vector<uint8_t>& out)
{
    if (in.empty() || in.size() % 96 != 0) return false;
    using Fq = grumpkin::fq; // the base field: BN254's r
    using Fr = grumpkin::fr; // the scalar field: BN254's p
    grumpkin::g1::element acc = grumpkin::g1::element::infinity();
    for (size_t i = 0; i < in.size(); i += 96) {
        uint256_t x = be(&in[i]), y = be(&in[i + 32]), s = be(&in[i + 64]);
        if (x >= Fq::modulus || y >= Fq::modulus) return false;
        grumpkin::g1::affine_element p;
        if (x == 0 && y == 0) p.self_set_infinity();
        else {
            p = grumpkin::g1::affine_element(Fq(x), Fq(y));
            if (!p.on_curve()) return false;
        }
        // any 32-byte scalar, reduced mod p (s < 2^256 < 6p)
        while (s >= Fr::modulus) s -= Fr::modulus;
        acc = acc + grumpkin::g1::element(p) * Fr(s);
    }
    grumpkin::g1::affine_element a(acc);
    out.assign(64, 0);
    if (!a.is_point_at_infinity()) { put(&out[0], uint256_t(a.x)); put(&out[32], uint256_t(a.y)); }
    return true;
}
int main()
{
    std::string res, ih, oh;
    int ok = 0, bad = 0;
    while (std::cin >> res >> ih >> oh) {
        std::vector<uint8_t> out;
        bool r = msm(unhex(ih == "-" ? "" : ih), out);
        bool agree = (res == "HUH") ? !r : (r && out == unhex(oh));
        (agree ? ok : bad)++;
        if (!agree) std::printf("DIFFER: %s %s\n", res.c_str(), ih.substr(0, 32).c_str());
    }
    std::printf("bb agrees on %d of %d call-105 vectors\n", ok, ok + bad);
    return bad != 0;
}
