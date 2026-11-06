# Native storylet host

The native examples load a compiled `RunContent` JSON document and drive the same deterministic run kernel as Play. The checked-in `run-content.json` is a small Weir's Crossing fixture. For another project, fetch `GET /api/v4/run-content` from its local Studio service and pass the saved JSON file as the first argument to either host.

## C ABI

Include `crates/storylet-engine/include/storylet_engine.h`. `dn_run_start` takes NUL-terminated UTF-8 JSON for content and start mode, plus a seed. On failure it returns a null handle and, when requested, an owned JSON error string. `dn_run_view` and `dn_run_act` return owned JSON strings; free each with `dn_string_free`. `dn_run_rewind` returns zero on success. Release the handle with `dn_run_free` and do not access one handle concurrently from multiple threads.

## Verify

On Windows with Rust and the .NET 8 SDK installed, run from the repository root:

```powershell
pwsh -NoProfile -File scripts/run-native-host.ps1
```

The .NET host loads the fixture and checks start, view, action, and rewind through the exported C ABI. To use another compiled document, build the Rust library and run the .NET project with the document path after `--`.

The C++ host uses the same API. Its CMake project links the Windows Rust DLL import library; build the Rust crate first, then configure and build `examples/native-host` with a C++ toolchain. Its built-in action targets the checked-in fixture's opening choice, so a different document needs a matching action supplied by the host.
