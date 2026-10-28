// The C and C++ runtime bb's verifier needs on the single-threaded rv64e PVM guest
// (aztec-on-jam#62), defined where the verifier may run it. Allocation, abort and
// chonk_trap come from the Rust service; stubs.cpp (generated) traps on the rest, which only
// logging, exceptions and files reach.
#include <chrono>
#include <cstddef>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <memory>
#include <mutex>
#include <new>
#include <string>

extern "C" [[noreturn]] void chonk_trap(const char* what);

extern "C" {
// -femulated-tls: each thread_local is a control block, and this returns the variable's
// storage, made on first use from the template (or zeroed). One thread, so one copy.
struct __emutls_control {
    size_t size;
    size_t align;
    void* object;
    void* value;
};
void* __emutls_get_address(__emutls_control* c)
{
    if (c->object == nullptr) {
        void* p = aligned_alloc(c->align < 16 ? 16 : c->align, c->size);
        if (c->value != nullptr) std::memcpy(p, c->value, c->size);
        else std::memset(p, 0, c->size);
        c->object = p;
    }
    return c->object;
}

// Function-local and inline-variable statics: one thread, so a guard is just its first byte.
int __cxa_guard_acquire(uint64_t* g) { return *reinterpret_cast<uint8_t*>(g) == 0; }
void __cxa_guard_release(uint64_t* g) { *reinterpret_cast<uint8_t*>(g) = 1; }
void __cxa_guard_abort(uint64_t*) {}

// Destructors of statics would run at exit; a refine has no exit, so they are not recorded.
int __cxa_atexit(void (*)(void*), void*, void*) { return 0; }
int __cxa_thread_atexit(void (*)(void*), void*, void*) { return 0; }

char* getenv(const char*) { return nullptr; }
unsigned long pthread_self(void) { return 1; }
int tolower(int c) { return c >= 'A' && c <= 'Z' ? c + 32 : c; }
}

void* operator new(size_t n) { return malloc(n ? n : 1); }
void* operator new[](size_t n) { return malloc(n ? n : 1); }
void* operator new(size_t n, const std::nothrow_t&) noexcept { return malloc(n ? n : 1); }
void* operator new(size_t n, std::align_val_t a) { return aligned_alloc(size_t(a), n ? n : 1); }
void* operator new[](size_t n, std::align_val_t a) { return aligned_alloc(size_t(a), n ? n : 1); }
void operator delete(void* p) noexcept { free(p); }
void operator delete[](void* p) noexcept { free(p); }
void operator delete(void* p, size_t) noexcept { free(p); }
void operator delete(void* p, std::align_val_t) noexcept { free(p); }
void operator delete[](void* p, std::align_val_t) noexcept { free(p); }
void operator delete(void* p, size_t, std::align_val_t) noexcept { free(p); }

namespace std {
const nothrow_t nothrow{};

inline namespace __1 {
// shared_ptr's control block, as libc++'s memory.cpp has it, one thread.
__shared_count::~__shared_count() {}
__shared_weak_count::~__shared_weak_count() {}
void __shared_weak_count::__release_weak() noexcept
{
    if (__shared_weak_owners_ == 0 || --__shared_weak_owners_ == -1) __on_zero_shared_weak();
}
const void* __shared_weak_count::__get_deleter(const type_info&) const noexcept { return nullptr; }

// One thread: a mutex has nothing to exclude.
void mutex::lock() {}
void mutex::unlock() noexcept {}
mutex::~mutex() {}

// The PVM has no clock; bb reads it only for its own timing scopes.
chrono::steady_clock::time_point chrono::steady_clock::now() noexcept { return time_point(); }

// unordered containers: any prime at least n, and any fixed hash.
size_t __next_prime(size_t n)
{
    if (n <= 2) return 2;
    for (size_t c = n | 1;; c += 2) {
        bool prime = true;
        for (size_t d = 3; d * d <= c; d += 2)
            if (c % d == 0) { prime = false; break; }
        if (prime) return c;
    }
}
size_t __hash_memory(_LIBCPP_NOESCAPE const void* p, size_t n) noexcept
{
    uint64_t h = 1469598103934665603ULL; // FNV-1a
    for (size_t i = 0; i < n; i++) h = (h ^ static_cast<const uint8_t*>(p)[i]) * 1099511628211ULL;
    return h;
}

template class basic_string<char>;
template string operator+ <char, char_traits<char>, allocator<char>>(const char*, const string&);

string to_string(unsigned long v)
{
    char b[24];
    int i = 24;
    do { b[--i] = char('0' + v % 10); v /= 10; } while (v);
    return string(b + i, b + 24);
}
string to_string(int v) { return v < 0 ? "-" + to_string(0UL - (unsigned long)(long)v) : to_string((unsigned long)v); }
} // namespace __1
} // namespace std
