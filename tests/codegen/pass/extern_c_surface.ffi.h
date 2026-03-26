#ifndef QLANG_EXTERN_C_SURFACE_FFI_H
#define QLANG_EXTERN_C_SURFACE_FFI_H

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

void q_host_log(const uint8_t* message);
int64_t q_host_add(int64_t left, int64_t right);
int64_t q_exported(int64_t value);

#ifdef __cplusplus
}
#endif

#endif /* QLANG_EXTERN_C_SURFACE_FFI_H */
