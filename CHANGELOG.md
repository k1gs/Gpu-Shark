# Changelog

## 0.2.4-beta.3

- Added automatic update checks: one anonymous HTTPS request to the public
  GitHub releases API at startup (User-Agent only, no machine data), with a
  Settings toggle to disable it.
- Added one-click update install: the release package is downloaded from the
  GitHub release, verified against SHA256SUMS.txt before unpacking, and
  applied with an atomic executable replacement that keeps a rollback backup.
- Nothing is downloaded or installed automatically; both steps require an
  explicit click in the About view.
- The public CI build now embeds the v0.2.4-beta.2 runtime by default.

## 0.2.4-beta.2

- Rejected the LibreHardwareMonitor Hot Spot and Memory Temperature channels
  on every GeForce RTX 50 card after a confirmed false RTX 5060 Ti reading
  (HotSpot 34.1 C at Core 34.0 C). The remaining LibreHardwareMonitor
  telemetry is unchanged.
- Extended the read-only GPC register HotSpot reader, validated on the
  desktop RTX 5050 (10DE:2D83), to the whole consumer RTX 50 lineup; the
  register map also decodes correctly on RTX 5060 Ti (GB206).
- Added a beta Memory Junction temperature channel from the private
  ThermChannel slot that is the memory sensor on every captured Blackwell
  board.
- On boards without a same-board validated profile the HotSpot falls back to
  an explicitly unverified beta aggregate and stays marked BETA; if no usable
  value exists, GPU Shark keeps showing N/A.

## 0.2.4-beta.1

- Added beta HotSpot support for the exact desktop GeForce RTX 5050
  10DE:2D83 profile after independent same-board validation.
- Marked displayed HotSpot rows on GeForce RTX 50-series cards as BETA;
  unavailable values remain unavailable.
- Documented the evidence-gated RTX 50 beta scope in the English and Russian
  README files and the support matrix.

## 0.2.3

- Fixed the feedback form so localized labels no longer overlap its input
  controls and both text fields follow the dark interface.
- Added a localized About view with the GPU Shark logo, application version,
  read-only statement and Beerware notice.
- Changed the top navigation to use measured GDI text widths, keeping visible
  spacing and matching click targets in both English and Russian.
- Made the public CI build produce the same standalone single executable by
  embedding a checksum-verified released runtime without publishing provider
  source or the production feedback endpoint.


## 0.2.2

- Added a persistent native Settings window for language, refresh interval and
  accent selection, with safe recovery from missing or malformed settings.
- Changed the default interface language to English while keeping the complete
  Russian localization, including the feedback experience.
- Moved the NVIDIA PerfCap reason into the regular sensor list and added a
  dedicated categorical detail view without a misleading numeric graph.
- Refreshed the project README with a clearer feature overview, supported-GPU
  table, roadmap, build guide and contribution workflow.
- Added AMD GPU support as a long-term idea without an active schedule.
## 0.2.1

- Fixed the standalone single-executable package so its embedded native provider
  is loaded from the private runtime cache and PerfCap remains available without
  placing DLL files beside `GPU-Shark.exe`.
- Changed embedded runtime cache validation from file-size checks to exact byte
  comparison, preventing stale same-size components after an update.
- Preserved the read-only hardware boundary and single-EXE user distribution.

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
