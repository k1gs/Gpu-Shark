# GPU Shark

[![Windows](https://img.shields.io/badge/Windows-10%20%7C%2011-0078D4?logo=windows)](https://github.com/k1gs/Gpu-Shark/releases)
[![Latest release](https://img.shields.io/github/v/release/k1gs/Gpu-Shark?display_name=tag)](https://github.com/k1gs/Gpu-Shark/releases/latest)
[![Binary distribution](https://img.shields.io/badge/source-closed-lightgrey)](LICENSE.md)

GPU Shark is a compact Windows terminal monitor for GPU and CPU telemetry. It
focuses on clear temperatures, a live HotSpot-to-Core delta and a dependency-
free user experience: unpack the release and run it.

![GPU Shark terminal interface](https://github.com/user-attachments/assets/92a2c325-cefb-4f2e-9e61-b5e9b12b539b)

## Telemetry

- GPU Core temperature
- GPU HotSpot temperature when available
- VRAM temperature when available
- VRAM usage
- Fan speed
- CPU package temperature
- Live `HotSpot - Core` delta with warnings

Unsupported sensors stay `N/A`; GPU Shark does not manufacture temperatures
from unrelated values.

## Download

1. Open the [latest release](https://github.com/k1gs/Gpu-Shark/releases/latest).
2. Download `GPU-Shark-win-x64.zip` and `SHA256SUMS.txt`.
3. Verify the archive hash, unpack it and run `GPU-Shark.exe` as Administrator.

```powershell
(Get-FileHash .\GPU-Shark-win-x64.zip -Algorithm SHA256).Hash
```

Windows SmartScreen may warn because the current binaries are not code-signed.
Always download releases from this repository and compare the published hash.

## Requirements

- Windows 10 or Windows 11, x64
- NVIDIA GPU and installed display driver
- Administrator privileges for local hardware access

The application performs local, read-only telemetry. It does not flash firmware,
change clocks or voltages, install a kernel driver, or send telemetry over the
network.

## GPU support

GPU Shark uses conservative per-device validation. Enhanced HotSpot support is
currently validated on selected RTX 20, RTX 30 and RTX 40 boards. RTX 50 support
is under active validation; unknown HotSpot values remain `N/A`.

See [SUPPORTED_GPUS.md](SUPPORTED_GPUS.md) for the current user-facing matrix.

## Distribution model

This public repository is the official documentation and binary release channel.
The application implementation and hardware research are not published. Release
packages contain optimized native binaries with no debug symbols or research
artifacts.

GPU Shark includes open-source components under their respective licenses. See
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). The GPU Shark binaries are
otherwise distributed under [LICENSE.md](LICENSE.md).

## Feedback

Bug reports are welcome through [GitHub Issues](https://github.com/k1gs/Gpu-Shark/issues).
Please include the GPU model, board vendor, driver version, GPU Shark version and
the visible symptom. Do not attach serial numbers, UUIDs or private machine data.
