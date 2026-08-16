# Security policy

## Authentic releases

Official binaries are published only under this repository's GitHub Releases.
Every release includes `SHA256SUMS.txt`. The binaries are currently unsigned, so
hash verification is strongly recommended.

## Runtime behavior

GPU Shark performs local read-only hardware telemetry. It does not install a
kernel driver, modify GPU settings or firmware, or upload collected data.

## Reporting

Report security problems through GitHub's private vulnerability reporting when
available. Otherwise open an issue asking the maintainer for a private contact
channel; do not post exploit details, machine identifiers, serial numbers or
private diagnostic archives publicly.

Only the latest release receives security fixes.
