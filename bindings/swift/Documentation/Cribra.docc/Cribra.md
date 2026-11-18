# ``Cribra``

Cribra provides native Swift APIs for sensitive-data scanning and explicit transformations.

## Overview

The Swift interface is generated from the Cribra Rust core through UniFFI.

The available API depends on the selected distribution profile:

- **Core:** scanning, findings, reports, streaming, and incremental scanning.
- **Flow:** Core capabilities plus transformations and Share Bundles.
- **Fuse:** Flow capabilities plus parallel scanning.

Use the API reference for the selected profile. Availability in one profile does not imply availability in every profile.

## Topics

### Scanning

- ``Scanner``
- ``ScanReport``
- ``Finding``

### Sessions

- ``StreamSession``
- ``IncrementalSession``
