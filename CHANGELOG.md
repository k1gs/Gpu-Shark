# Changelog

## 0.2.0

- Promoted the native GUI line to the first stable `0.2.0` release.
- Added GPU-Z-compatible decoding of NVIDIA PerfCap reasons: `Pwr`, `Thrm`,
  `VRel`, `VOp`, `Idle` and `SLI`, including combined active reasons.
- Added stable sensor identities so selection, history and maximum tracking
  survive known provider display-name changes.
- Kept unsupported sensors hidden and the unreliable Memory Clock disabled.
- Updated the matching public runtime contract while preserving read-only
  operation and the existing consent-based feedback boundary.

## 0.2.0-beta.2

- Reworked the native interface into a fixed landscape sensor dashboard with
  clearer section separation and flicker-free updates.
- Added a selected-sensor graph with a tighter scale and optional maximum-value
  tracking enabled by double-clicking a sensor row.
- Added separate fan RPM, power, voltage and performance-limit readings when
  provided by the installed GPU.
- Moved secondary GPU and system readings below the primary graphics-card
  sensors and stopped displaying unavailable rows.
- Removed the unreliable memory-clock reading while its provider semantics are
  reviewed.
- Added Russian interface text and an in-app language switch.
- Added a consent-based feedback form with privacy-filtered public telemetry,
  report IDs and explicit handling for validation, size, rate-limit and server
  errors.
- Added a standalone single-executable distribution and fixed stale embedded
  runtime replacement, window repaint flicker, icon fallback and graph hit
  testing.
- Published the native Rust/Win32 GUI client source under the Beerware license;
  telemetry-provider DLL implementations remain outside the public source tree.

## 0.2.0-beta.1

- Added the first native Windows GUI with a dark interface.
- Added the detected NVIDIA GPU model to the telemetry view.
- Added automatic Windows administrator elevation through the application
  manifest.
- Fixed component loading so bundled DLLs are resolved beside
  `GPU-Shark.exe`, independent of the working directory.
- Changed the public package to the GUI application only.
- Marked this release as beta while the new interface is validated.

## 0.1.4

- Added validated GeForce RTX 3080 and RTX 4090 board profiles.
- Added HotSpot support for both profiles and VRAM temperature support for the
  validated RTX 3080 profile.
- Kept the unverified RTX 4090 VRAM-temperature channel disabled.
- Documented experimental GeForce RTX 5070 Ti Core and VRAM-temperature
  support on a tested board.
- Kept the unvalidated RTX 5070 Ti HotSpot value unavailable.

## 0.1.3

- Added a validated GeForce RTX 3050 board profile.
- Added exact HotSpot handling for the validated RTX 3050 profile.

## 0.1.2

- Added a validated GeForce RTX 3060 board profile.
- Added exact HotSpot handling for the validated RTX 3060 profile.

## 0.1.1

- Added a validated GeForce RTX 2070 SUPER board profile.
- Added HotSpot and VRAM temperature support for the validated RTX 2070 SUPER profile.

## 0.1.0

- Added conservative enhanced HotSpot support for validated RTX 20/30/40 boards.
- Added initial RTX 50 Core and VRAM telemetry handling.
- Added live HotSpot-to-Core delta monitoring.
- Rebuilt the application as optimized x64 native binaries.
- Introduced a hardened binary-only public distribution with stripped symbols.
- Added SHA-256 verification and third-party notices.

## 0.0.1

- Initial public preview.
