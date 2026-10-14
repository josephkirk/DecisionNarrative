# Native storylet host

This example demonstrates hosting a complete, generic storylet scenario from a native game process. `storylet-project.json` is an Archive Mystery fixture, not a built-in game world: the host loads it as project data, asks the Rust director to evaluate it, executes the selected choice, and reads the resulting world snapshot.

## ABI contract

The public C header is `crates/storylet-engine/include/storylet_engine.h`. The ABI accepts UTF-8 JSON buffers and returns UTF-8 JSON in `DnBuffer`. Inputs are borrowed until the function returns. Every returned buffer, including error results, must be released exactly once with `dn_buffer_free`. Check the `ok` field and then read either `result` or `error`. Destroy each handle with its matching destroy function. A `DnPlaytestHandle` owns one active scenario; create a new handle to host a different scenario. The ABI version is reported by `dn_engine_abi_version()`.

## Verify the ABI

On Windows with Rust and the .NET 8 SDK installed, run from the repository root:

```powershell
pnpm smoke:native-host
```

The .NET console app uses P/Invoke against the exported C ABI and exercises scenario load, director evaluation, choice execution, and snapshot retrieval. This is the automated cross-language host check.

## C++ / Unreal integration

`main.cpp` is a small C++ host and `CMakeLists.txt` builds it against the generated Windows import library. Build the `storylet-engine` Rust crate first, then configure CMake with a C++ toolchain. Unreal modules can include the same C header and link the platform-specific library produced by Cargo; keep the engine handle owned by the game subsystem and pass scenario/project JSON from game content. The C++ sample has not been compiled in environments without a C++ compiler.
