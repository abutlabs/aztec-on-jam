// The CRS in bb's in-memory form, for chonk_pvm_verify_crs (shim.cpp), written by bb itself: each
// point read from bb's CRS files with bb's own from_buffer, then its bytes copied out as they sit
// in memory (Montgomery form, little-endian limbs: the same on arm64, x86-64 and the PVM's rv64e).
//   crs_raw <bb crs dir> <n_bn254> <n_grumpkin> <out>
#include "barretenberg/common/serialize.hpp"
#include "barretenberg/ecc/curves/bn254/g1.hpp"
#include "barretenberg/ecc/curves/bn254/g2.hpp"
#include "barretenberg/ecc/curves/grumpkin/grumpkin.hpp"
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <iterator>
#include <string>
#include <vector>

static std::vector<uint8_t> slurp(const std::string& p)
{
    std::ifstream f(p, std::ios::binary);
    return { std::istreambuf_iterator<char>(f), {} };
}
template <typename T> static void put(std::vector<uint8_t>& out, const T& v)
{
    const auto* b = reinterpret_cast<const uint8_t*>(&v);
    out.insert(out.end(), b, b + sizeof(T));
}
static void put_u32(std::vector<uint8_t>& out, uint32_t v)
{
    for (int i = 0; i < 4; i++) out.push_back(uint8_t(v >> (8 * i)));
}

int main(int argc, char** argv)
{
    using namespace bb;
    if (argc != 5) { std::fprintf(stderr, "usage: crs_raw <crs dir> <n_bn254> <n_grumpkin> <out>\n"); return 2; }
    std::string dir = argv[1];
    uint32_t n_bn = uint32_t(std::atoi(argv[2])), n_gr = uint32_t(std::atoi(argv[3]));
    auto bn = slurp(dir + "/bn254_g1.dat"), g2 = slurp(dir + "/bn254_g2.dat"), gr = slurp(dir + "/grumpkin_g1_v2.flat.dat");
    if (bn.size() < size_t(n_bn) * 64 || g2.size() < 128 || gr.size() < size_t(n_gr) * 64) { std::fprintf(stderr, "crs files too short\n"); return 1; }
    std::vector<uint8_t> out;
    put_u32(out, n_bn);
    for (size_t i = 0; i < n_bn; i++) put(out, from_buffer<g1::affine_element>(bn, i * 64));
    put(out, from_buffer<g2::affine_element>(g2.data()));
    put_u32(out, n_gr);
    for (size_t i = 0; i < n_gr; i++) put(out, from_buffer<curve::Grumpkin::AffineElement>(gr, i * 64));
    std::ofstream(argv[4], std::ios::binary).write(reinterpret_cast<const char*>(out.data()), std::streamsize(out.size()));
    std::printf("%s: %u BN254 points, G2, %u Grumpkin points, %zu bytes (sizeof g1 %zu, g2 %zu, grumpkin %zu)\n", argv[4], n_bn, n_gr,
                out.size(), sizeof(g1::affine_element), sizeof(g2::affine_element), sizeof(curve::Grumpkin::AffineElement));
    return 0;
}
