# Building the Windows binary

The phase-1 target is `x86_64-pc-windows-gnu`. Two paths:

## On Windows

```
rustup target add x86_64-pc-windows-gnu        # or use the native -msvc toolchain
cargo build --release
# binary at target\release\phantom.exe
```

WebView2 is part of Windows 11 / current Windows 10; no extra SDK is
needed for a gnu build beyond the standard mingw-w64 toolchain
(`winget install MartinStorsjo.LLVM-MinGW` or MSYS2 `mingw-w64-x86_64-gcc`).

## Cross-building from Linux

```
rustup target add x86_64-pc-windows-gnu
# any mingw-w64 providing x86_64-w64-mingw32-gcc works; llvm-mingw is fine too
apt install mingw-w64
cargo build --release --target x86_64-pc-windows-gnu
# binary at target/x86_64-pc-windows-gnu/release/phantom.exe
```

Notes:

- `rusqlite` uses the bundled SQLite, so a C compiler for the target must
  be on PATH; the `cc` crate picks up `x86_64-w64-mingw32-gcc`.
- The release profile enables thin LTO and symbol stripping; expect a
  few minutes on two cores.
- The produced `.exe` is standalone; WebView2 runtime is the only runtime
  dependency and ships with modern Windows.
