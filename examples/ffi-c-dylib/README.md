# C Host Shared-Library FFI Example

This example shows the current conservative `dylib` host workflow for Qlang.

It demonstrates two boundaries at once:

- A C host loads a Qlang shared library at runtime and calls an exported
  `extern "c"` function.
- The Qlang module may still contain internal async helpers, but the exported C
  ABI surface remains synchronous.

## Layout

- `ql/callback_add.ql`: Qlang shared-library source
- `host/main.c`: standalone C host that loads the built shared library and
  resolves `q_add` dynamically

## Run

From the repository root on macOS/Linux:

```bash
mkdir -p examples/ffi-c-dylib/build
cargo run -p ql-cli -- build examples/ffi-c-dylib/ql/callback_add.ql --emit dylib --output examples/ffi-c-dylib/build/libcallback_add.so
clang examples/ffi-c-dylib/host/main.c -o examples/ffi-c-dylib/build/ffi-c-dylib-host -ldl
./examples/ffi-c-dylib/build/ffi-c-dylib-host examples/ffi-c-dylib/build/libcallback_add.so
```

On macOS, replace the library suffix with `.dylib`.

On Windows PowerShell:

```powershell
New-Item -ItemType Directory -Force examples/ffi-c-dylib/build | Out-Null
cargo run -p ql-cli -- build examples/ffi-c-dylib/ql/callback_add.ql --emit dylib --output examples/ffi-c-dylib/build/callback_add.dll
clang examples/ffi-c-dylib/host/main.c -o examples/ffi-c-dylib/build/ffi-c-dylib-host.exe
.\examples\ffi-c-dylib\build\ffi-c-dylib-host.exe .\examples\ffi-c-dylib\build\callback_add.dll
```

The host-visible API is intentionally narrow: current `dylib` support keeps the
public C surface on synchronous `extern "c"` exports even when the module also
contains internal async helpers.
