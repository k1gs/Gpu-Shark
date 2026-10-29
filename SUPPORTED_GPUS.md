# Supported GPUs

Support is tracked per physical-device profile. A family name below does not
imply that every board variant exposes the same sensors.

| GPU | Core | HotSpot | VRAM temperature | Status |
|---|---:|---:|---:|---|
| GeForce RTX 2060 SUPER | Yes | Validated | Board-dependent | Validated board profile |
| GeForce RTX 2070 SUPER | Yes | Validated | Validated | Validated board profile |
| GeForce RTX 3050 | Yes | Validated | Board-dependent | Validated board profile |
| GeForce RTX 3060 | Yes | Validated | Board-dependent | Validated board profile |
| GeForce RTX 3070 | Yes | Validated | Board-dependent | Validated board profile |
| GeForce RTX 3080 | Yes | Validated | Validated | Validated board profile |
| GeForce RTX 4060 | Yes | Validated | Board-dependent | Validated board profile |
| GeForce RTX 4090 | Yes | Validated | Under validation | Validated HotSpot profile |
| GeForce RTX 5050 | Yes | Beta | Beta | GPC register HotSpot profile |
| GeForce RTX 5060 Ti | Yes | Beta | Beta | GPC register HotSpot profile |
| GeForce RTX 5070 | Yes | Beta | Beta | GPC register HotSpot profile |
| GeForce RTX 5070 Ti | Yes | Beta | Beta | GPC register HotSpot profile |
| GeForce RTX 5090 | Yes | Validated | Validated | Fully validated on tested hardware |
| RTX PRO 6000 Blackwell | Yes | Validated | Validated | Fully validated on tested hardware |
| Other NVIDIA GPUs | Usually | Driver-dependent | Driver/board-dependent | Conservative fallback |

The RTX 5090 and RTX PRO 6000 Blackwell entries reflect maintainer-confirmed
full application validation on the tested hardware; they do not extend sensor
mappings to other board variants.

When a value cannot be validated, GPU Shark leaves it unavailable. This is intentional.
Open an issue with the exact GPU model, board vendor, driver version and VBIOS
version if a sensor that should exist is missing.
