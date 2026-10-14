$ErrorActionPreference = "Stop"

cargo build --manifest-path crates/storylet-engine/Cargo.toml --lib
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

dotnet run --project examples/native-host-dotnet/NativeHost.csproj
exit $LASTEXITCODE
