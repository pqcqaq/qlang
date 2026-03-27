# Rust Host FFI Example

This example keeps the Qlang <-> Rust interop surface anchored on the current stable C ABI.

It demonstrates two things at once:

- Rust calls a Qlang-exported `extern "c"` function.
- Qlang calls back into a Rust-provided `extern "C"` host function.

## Layout

- `ql/callback_add.ql`: Qlang library source compiled as a `staticlib`
- `host/`: standalone Cargo host project that builds and links the Qlang library through `build.rs`

## Run

From the repository root:

```bash
cargo run --manifest-path examples/ffi-rust/host/Cargo.toml
```

If `ql` is not installed on your `PATH`, point the example at a built local compiler binary:

```bash
QLANG_BIN=target/debug/ql cargo run --manifest-path examples/ffi-rust/host/Cargo.toml
```

On Windows PowerShell:

```powershell
$env:QLANG_BIN = (Resolve-Path .\target\debug\ql.exe)
cargo run --manifest-path examples/ffi-rust/host/Cargo.toml
```

If your archive tool is not already discoverable, also set `QLANG_AR` (and `QLANG_AR_STYLE` when using a wrapper path whose name does not imply `ar` vs `lib` style).
