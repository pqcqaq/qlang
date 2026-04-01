#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>

#include "callback_add.ffi.h"

int64_t q_host_add(int64_t left, int64_t right) {
    return left + right;
}

int64_t q_host_multiply(int64_t left, int64_t right) {
    return left * right;
}

int main(void) {
    const int64_t add_result = q_add_two(40);
    const int64_t scale_result = q_scale(6, 7);

    if (add_result != 42) {
        fprintf(stderr, "expected q_add_two(40) == 42, got %" PRId64 "\n", add_result);
        return 1;
    }

    if (scale_result != 42) {
        fprintf(stderr, "expected q_scale(6, 7) == 42, got %" PRId64 "\n", scale_result);
        return 1;
    }

    printf("q_add_two(40) = %" PRId64 "\n", add_result);
    printf("q_scale(6, 7) = %" PRId64 "\n", scale_result);
    return 0;
}
