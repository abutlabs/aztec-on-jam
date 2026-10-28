/* mmap declarations only, for the rv64e PVM guest (aztec-on-jam#62); bb maps files
   only in its low-memory mode, which the verifier does not use. */
#ifndef _PVM_SYS_MMAN_H
#define _PVM_SYS_MMAN_H
#include <sys/types.h>
#define PROT_READ 1
#define PROT_WRITE 2
#define MAP_SHARED 1
#define MAP_PRIVATE 2
#define MAP_FAILED ((void*)-1)
#ifdef __cplusplus
extern "C" {
#endif
void* mmap(void*, size_t, int, int, int, off_t);
int munmap(void*, size_t);
#ifdef __cplusplus
}
#endif
#endif
