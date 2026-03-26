#include "extern_c_export.h"

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
    int status;

    if (argc != 2) {
        return 2;
    }

    library = ql_open_library(argv[1]);
    if (library == NULL) {
        return 3;
    }

    q_add_dynamic = (q_add_fn)ql_load_symbol(library, "q_add");
    if (q_add_dynamic == NULL) {
        ql_close_library(library);
        return 4;
    }

    status = q_add_dynamic(20, 22) == 42 ? 0 : 1;
    ql_close_library(library);
    return status;
}
