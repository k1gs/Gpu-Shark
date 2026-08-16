# Supported GPUs

Support is tracked per physical-device profile. A family name below does not
imply that every board variant exposes the same sensors.

| GPU | Core | HotSpot | VRAM temperature | Status |
|---|---:|---:|---:|---|
| GeForce RTX 2060 SUPER | Yes | Validated | Board-dependent | Validated board profile |
| GeForce RTX 3070 | Yes | Validated | Board-dependent | Validated board profile |
| GeForce RTX 4060 | Yes | Validated | Board-dependent | Validated board profile |
| GeForce RTX 5070 | Yes | Not exposed | Yes on tested board | Experimental RTX 50 support |
| Other NVIDIA GPUs | Usually | Driver-dependent | Driver/board-dependent | Conservative fallback |

When a value cannot be validated, GPU Shark displays `N/A`. This is intentional.
Open an issue with the exact GPU model, board vendor, driver version and VBIOS
version if a sensor that should exist is missing.
