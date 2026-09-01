# GPU Shark GUI source

This directory contains the public Rust/Win32 GUI client. It includes rendering,
stable sensor identities and history, GPU-Z-compatible presentation of decoded
NVIDIA PerfCap reasons, Russian and English UI strings, and the consent-based
feedback client.

The telemetry provider itself is not part of this source tree. Use all prebuilt
DLLs from the matching `GPU-Shark-gui-runtime-win-x64.zip` release asset. They
may stay beside a development build or be embedded into a standalone EXE.
Mixing incompatible GUI and runtime versions is unsupported.

## Build

Install the stable Rust toolchain and Visual Studio C++ build tools, then run
PowerShell from this directory:

```powershell
$env:GPU_SHARK_FEEDBACK_HOST = "your-compatible-feedback-host"
$env:GPU_SHARK_FEEDBACK_PATH = "/your/api/path"
cargo build --release
Copy-Item .\runtime\*.dll .\target\release\
```

To reproduce the standalone packaging after verifying the runtime archive
against the release checksum:

```powershell
$env:GPU_SHARK_EMBED_PUBLIC_RUNTIME = "1"
$env:GPU_SHARK_PUBLIC_PAYLOAD_DIR = "C:\path\to\verified-runtime"
cargo build --release --locked
```

The standalone executable extracts its exact embedded DLL bytes into a
versioned per-user runtime cache and prefers adjacent DLLs for development.

The official production endpoint is intentionally release-time configuration;
it is not stored in this repository. This is not a secrecy boundary: an endpoint
used by a desktop application can be observed in the binary or network traffic.
The server must enforce payload validation, rate limits and abuse protection.

The public ABI is read-only. It returns user-facing telemetry through `gs.dll`
export `q`; the GUI does not contain hardware-control features.

The GUI source is covered by the repository Beerware license. Rust dependencies
and the prebuilt runtime retain their respective notices supplied with release
assets.
