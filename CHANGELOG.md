# Changelog

## Unreleased

- Documented experimental GeForce RTX 5070 Ti Core and VRAM-temperature
  support on a tested board.
- Kept the unvalidated RTX 5070 Ti HotSpot value unavailable.

## 0.1.4

- Added validated GeForce RTX 3080 and RTX 4090 board profiles.
- Added HotSpot support for both profiles and VRAM temperature support for the
  validated RTX 3080 profile.
- Kept the unverified RTX 4090 VRAM-temperature channel disabled.

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
