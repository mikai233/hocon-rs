# Changelog

## [0.2.0] - 2026-08-13

### Added

- Added optional Java properties support through the `properties` feature.
- Added include depth limits and support for relative file and URL includes.

### Changed

- Separated include syntax parsing from file and URL loading.
- Disabled Java properties support by default.
- Upgraded all dependencies, including `reqwest` 0.13 and `num-bigint` 0.5.

### Fixed

- Improved include cycle detection, HTTP error handling, content type detection, and `file://` URL handling.

## [0.1.3] - 2025-10-03

### Fixed

- Fixed a potential substitution stack overflow.
- Fixed handling of U+FEFF (Zero Width No-Break Space) characters.

## [0.1.2] - 2025-09-28

### Fixed

- Fixed a panic in HOCON parser caused by invalid Unicode escape sequences.
- Improved error handling for malformed surrogate pairs in `\uXXXX` escapes.

## [0.1.1] - 2025-09-25

### Changed

- Optimized parsing logic: parse directly from raw bytes instead of decoding into UTF-8.
    - Avoids unnecessary memory allocations and copies.
    - Improves performance, especially for large configuration files.
