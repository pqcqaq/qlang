#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>

#if defined(_WIN32)
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
typedef HMODULE ql_library_handle;

static ql_library_handle ql_open_library(const char *path) {
    return LoadLibraryA(path);
}

static FARPROC ql_load_symbol(ql_library_handle library, const char *name) {
    return GetProcAddress(library, name);
}

static void ql_close_library(ql_library_handle library) {
    if (library != NULL) {
        FreeLibrary(library);
    }
}
#else
#include <dlfcn.h>
typedef void *ql_library_handle;

static ql_library_handle ql_open_library(const char *path) {
    return dlopen(path, RTLD_NOW);
}

static void *ql_load_symbol(ql_library_handle library, const char *name) {
    return dlsym(library, name);
}

static void ql_close_library(ql_library_handle library) {
    if (library != NULL) {
        dlclose(library);
    }
}
#endif

typedef int64_t (*q_add_fn)(int64_t left, int64_t right);

int main(int argc, char **argv) {
    ql_library_handle library;
    q_add_fn q_add_dynamic;
    int64_t result;

    if (argc != 2) {
        fprintf(stderr, "expected shared library path argument\n");
        return 2;
    }

    library = ql_open_library(argv[1]);
    if (library == NULL) {
        fprintf(stderr, "failed to open shared library: %s\n", argv[1]);
        return 3;
    }

    q_add_dynamic = (q_add_fn)ql_load_symbol(library, "q_add");
    if (q_add_dynamic == NULL) {
        fprintf(stderr, "failed to resolve symbol: q_add\n");
        ql_close_library(library);
        return 4;
    }

    result = q_add_dynamic(20, 22);
    if (result != 42) {
        fprintf(stderr, "expected q_add(20, 22) == 42, got %" PRId64 "\n", result);
        ql_close_library(library);
        return 1;
    }

    printf("q_add(20, 22) = %" PRId64 "\n", result);
    ql_close_library(library);
    return 0;
}
