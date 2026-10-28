// Native harness for shim.cpp: reads a payload file, runs a mode N times, prints the verdict and time.
// With a fourth argument, the payload is [proof][vk] only and the CRS comes from that file in bb's
// in-memory form (crs_raw), through chonk_pvm_verify_crs.
#include <chrono>
#include <cstdio>
#include <cstdint>
#include <cstdlib>
#include <vector>
extern "C" int chonk_pvm_verify(const uint8_t*, size_t, int);
extern "C" int chonk_pvm_verify_crs(const uint8_t*, size_t, const uint8_t*, size_t, int);
extern "C" int chonk_pvm_verify_batch(const uint8_t*, size_t, const uint8_t*, size_t, uint8_t*, size_t);
static std::vector<uint8_t> slurp(const char* path)
{
    FILE* f = std::fopen(path, "rb");
    std::vector<uint8_t> b;
    uint8_t buf[65536]; size_t n;
    while ((n = std::fread(buf, 1, sizeof buf, f)) > 0) b.insert(b.end(), buf, buf + n);
    std::fclose(f);
    return b;
}
int main(int argc, char** argv)
{
    std::vector<uint8_t> b = slurp(argv[1]), crs;
    if (argc > 4) crs = slurp(argv[4]);
    int mode = std::atoi(argv[2]), reps = argc > 3 ? std::atoi(argv[3]) : 1, v = 0;
    if (mode == 9) {   // a batch (pv.py's batch file) against crs_raw's CRS
        uint8_t verdicts[64] = {};
        auto t0 = std::chrono::steady_clock::now();
        for (int i = 0; i < reps; i++) v = chonk_pvm_verify_batch(b.data(), b.size(), crs.data(), crs.size(), verdicts, 64);
        double ms = std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now() - t0).count() / reps;
        std::printf("batch of %d: verdicts", v);
        for (int i = 0; i < v; i++) std::printf(" %d", verdicts[i]);
        std::printf(" %.2f ms\n", ms);
        return 0;
    }
    auto t0 = std::chrono::steady_clock::now();
    for (int i = 0; i < reps; i++) v = crs.empty() ? chonk_pvm_verify(b.data(), b.size(), mode) : chonk_pvm_verify_crs(b.data(), b.size(), crs.data(), crs.size(), mode);
    double ms = std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now() - t0).count() / reps;
    std::printf("mode %d verdict %d %.2f ms\n", mode, v, ms);
    return 0;
}
