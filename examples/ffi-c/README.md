# C Host FFI Example

This example keeps the Qlang <-> C interop surface anchored on the current
stable C ABI and `staticlib` workflow.

It demonstrates two things at once:

- A C host calls Qlang-exported `extern "c"` functions.
- Qlang calls back into C-provided `extern "C"` host functions.

## Layout

- `ql/callback_add.ql`: Qlang library source compiled as a `staticlib`
- `host/main.c`: standalone C host that provides callback functions and calls
  Qlang exports through the generated combined header

## Run

From the repository root on macOS/Linux:

```bash
mkdir -p examples/ffi-c/build
cargo run -p ql-cli -- build examples/ffi-c/ql/callback_add.ql --emit staticlib --output examples/ffi-c/build/libcallback_add.a --header-surface both --header-output examples/ffi-c/build/callback_add.ffi.h
clang -I examples/ffi-c/build examples/ffi-c/host/main.c examples/ffi-c/build/libcallback_add.a -o examples/ffi-c/build/ffi-c-host
./examples/ffi-c/build/ffi-c-host
```

On Windows PowerShell:

```powershell
New-Item -ItemType Directory -Force examples/ffi-c/build | Out-Null
cargo run -p ql-cli -- build examples/ffi-c/ql/callback_add.ql --emit staticlib --output examples/ffi-c/build/callback_add.lib --header-surface both --header-output examples/ffi-c/build/callback_add.ffi.h
clang -I examples/ffi-c/build examples/ffi-c/host/main.c examples/ffi-c/build/callback_add.lib -o examples/ffi-c/build/ffi-c-host.exe
.\examples\ffi-c\build\ffi-c-host.exe
```

If your archive tool or clang executable is not already discoverable, also set
`QLANG_AR` / `QLANG_AR_STYLE` and `QLANG_CLANG` before running `ql build`.
