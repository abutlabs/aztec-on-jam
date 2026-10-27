// Native harness for shim.cpp: reads a payload file, runs a mode N times, prints the verdict and time.
#include <chrono>
#include <cstdio>
#include <cstdint>
#include <cstdlib>
#include <vector>
extern "C" int chonk_pvm_verify(const uint8_t*, size_t, int);
int main(int argc, char** argv)
{
    FILE* f = std::fopen(argv[1], "rb");
    std::vector<uint8_t> b;
    uint8_t buf[65536]; size_t n;
    while ((n = std::fread(buf, 1, sizeof buf, f)) > 0) b.insert(b.end(), buf, buf + n);
    int mode = std::atoi(argv[2]), reps = argc > 3 ? std::atoi(argv[3]) : 1, v = 0;
    auto t0 = std::chrono::steady_clock::now();
    for (int i = 0; i < reps; i++) v = chonk_pvm_verify(b.data(), b.size(), mode);
    double ms = std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now() - t0).count() / reps;
    std::printf("mode %d verdict %d %.2f ms\n", mode, v, ms);
    return 0;
}
