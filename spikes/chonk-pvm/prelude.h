/* Forced first include for bb on the rv64e PVM guest (aztec-on-jam#62): what
   bb's common/net.hpp includes on Linux and wasm, so bb's sources stay unchanged. */
#pragma once
#include <arpa/inet.h>
#include <endian.h>
#define ntohll be64toh
#define htonll htobe64
#ifdef __cplusplus
#include <cstdlib>
/* bb's common/mem.hpp defines aligned_free for Linux, Apple, wasm and Windows; Linux's is free. */
inline void aligned_free(void* mem) { free(mem); }
#endif
