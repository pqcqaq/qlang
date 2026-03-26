#include "extern_c_import_block.ffi.h"

int64_t q_host_add(int64_t left, int64_t right) {
    return left + right;
}

int main(void) {
    return q_add_two(40) == 42 ? 0 : 1;
}
