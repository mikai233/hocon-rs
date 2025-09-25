# Changelog

## [0.1.1] - 2025-09-25

### Changed

- Optimized parsing logic: parse directly from raw bytes instead of decoding into UTF-8.
    - Avoids unnecessary memory allocations and copies.
    - Improves performance, especially for large configuration files.
