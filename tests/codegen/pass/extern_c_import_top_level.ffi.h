#ifndef QLANG_EXTERN_C_IMPORT_TOP_LEVEL_FFI_H
#define QLANG_EXTERN_C_IMPORT_TOP_LEVEL_FFI_H

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

int64_t q_host_add(int64_t left, int64_t right);
int64_t q_add_two(int64_t value);

#ifdef __cplusplus
}
#endif

#endif /* QLANG_EXTERN_C_IMPORT_TOP_LEVEL_FFI_H */
