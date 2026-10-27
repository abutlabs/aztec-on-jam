/* Single-threaded pthread declarations for the rv64e PVM guest (jam-privacy-service#62).
   The guest has one thread; the link supplies no-op implementations. */
#ifndef _PVM_PTHREAD_H
#define _PVM_PTHREAD_H
#include <sched.h>
#include <time.h>
#ifdef __cplusplus
extern "C" {
#endif
int sched_yield(void);
typedef unsigned long pthread_t;
typedef struct { int kind; } pthread_mutexattr_t;
typedef struct { int kind; int count; } pthread_mutex_t;
typedef struct { int unused; } pthread_condattr_t;
typedef struct { int unused; } pthread_cond_t;
typedef struct { int unused; } pthread_attr_t;
typedef unsigned pthread_key_t;
typedef int pthread_once_t;
#define PTHREAD_MUTEX_INITIALIZER { 0, 0 }
#define PTHREAD_COND_INITIALIZER { 0 }
#define PTHREAD_ONCE_INIT 0
#define PTHREAD_MUTEX_NORMAL 0
#define PTHREAD_MUTEX_RECURSIVE 1
int pthread_mutexattr_init(pthread_mutexattr_t*);
int pthread_mutexattr_settype(pthread_mutexattr_t*, int);
int pthread_mutexattr_destroy(pthread_mutexattr_t*);
int pthread_mutex_init(pthread_mutex_t*, const pthread_mutexattr_t*);
int pthread_mutex_lock(pthread_mutex_t*);
int pthread_mutex_trylock(pthread_mutex_t*);
int pthread_mutex_unlock(pthread_mutex_t*);
int pthread_mutex_destroy(pthread_mutex_t*);
int pthread_cond_init(pthread_cond_t*, const pthread_condattr_t*);
int pthread_cond_signal(pthread_cond_t*);
int pthread_cond_broadcast(pthread_cond_t*);
int pthread_cond_wait(pthread_cond_t*, pthread_mutex_t*);
int pthread_cond_timedwait(pthread_cond_t*, pthread_mutex_t*, const struct timespec*);
int pthread_cond_destroy(pthread_cond_t*);
int pthread_once(pthread_once_t*, void (*)(void));
int pthread_create(pthread_t*, const pthread_attr_t*, void* (*)(void*), void*);
int pthread_join(pthread_t, void**);
int pthread_detach(pthread_t);
pthread_t pthread_self(void);
int pthread_equal(pthread_t, pthread_t);
int pthread_key_create(pthread_key_t*, void (*)(void*));
int pthread_key_delete(pthread_key_t);
void* pthread_getspecific(pthread_key_t);
int pthread_setspecific(pthread_key_t, const void*);
#ifdef __cplusplus
}
#endif
#endif
