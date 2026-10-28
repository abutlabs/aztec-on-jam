/* Optional forced include (build.sh PLAIN_INLINE=1, aztec-on-jam#62): bb's
   BB_INLINE forces inlining of its field arithmetic, which makes the rv64e code larger than
   the 4 MB service-code limit. Defining it as plain `inline` after bb's own definition
   (compiler_hints.hpp is #pragma once) leaves inlining to LLVM's cost model. */
#pragma once
#include "barretenberg/common/compiler_hints.hpp"
#undef BB_INLINE
#define BB_INLINE inline
