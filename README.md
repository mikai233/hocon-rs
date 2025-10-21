# hocon-rs

A **Rust implementation of [HOCON](https://github.com/lightbend/config/blob/main/HOCON.md)**  
(Human-Optimized Config Object Notation), with full spec compliance, `serde` integration, and support for advanced
features like substitutions and includes.

[![codecov](https://codecov.io/gh/mikai233/hocon-rs/branch/master/graph/badge.svg?token=KJ3YM1FNXX)](https://codecov.io/gh/mikai233/hocon-rs)
[![Build Status](https://github.com/mikai233/hocon-rs/actions/workflows/rust.yml/badge.svg)](https://github.com/mikai233/hocon-rs/actions)
[![crates.io](https://img.shields.io/crates/v/hocon-rs.svg)](https://crates.io/crates/hocon-rs)
[![Docs](https://docs.rs/hocon-rs/badge.svg)](https://docs.rs/hocon-rs)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
---

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
hocon-rs = "0.1"
```

# Quick Start

## Load configuration from a file

```hocon
# app.conf - Basic configuration
app-name = "My Application"
version = "1.0"

# Database configuration
database {
  host = "localhost"
  port = 5432
  name = "mydb"
}

# Server configuration
server {
  port = 8080
  timeout = 30s  # 30 seconds
}

# Feature flags
features {
  logging = true
  cache = false
}
```

```rust
fn main() -> Result<(), hocon_rs::Error> {
  let value: hocon_rs::Value = hocon_rs::Config::load("application.conf", None)?;
  let host = value.get_by_path(["database", "host"]).unwrap();
  println!("{}", host);
  Ok(())
}
```

## Load configuration from a string

```rust
fn main() -> Result<(), hocon_rs::Error> {
  let value: hocon_rs::Value = hocon_rs::Config::from_str("{name = mikai233}", None)?;
  println!("{value}");
  Ok(())
}
```

## Deserialize into a struct using `serde`

```rust
#[derive(Debug, Deserialize)]
struct Person {
  name: String,
  age: u32,
  scores: Vec<i32>,
}

fn main() -> Result<(), hocon_rs::Error> {
  let person: Person = hocon_rs::Config::from_str("{name = mikai233, age = 18, scores = [99, 100]}", None)?;
  println!("{person:?}");
  Ok(())
}
```

# Integration with Serde JSON

This library depends on `serde_json` for JSON deserialization.
`hocon_rs::Value` implements conversions to and from `serde_json::Value`, making it easy to interoperate with other
serde-compatible libraries.

# Important Notes

## Project Status

This library is still under active development.
Most features are already implemented, but the public API may still change in future versions.

## About Classpath

In HOCON, configurations can be loaded from the **classpath**.
Since classpath is a Java-specific concept, the Rust implementation defines the classpath as a set of directory roots
used to search for configuration files.

If you do not configure a classpath in `ConfigOptions`, `hocon-rs` will only search in the current working directory.

## Object And Array Depth Limit

When parsing deeply nested objects or arrays, you may encounter a `RecursionDepthExceeded` error.
This happens because `hocon-rs` uses recursive functions to parse objects, and excessive recursion could cause stack
overflows.

The default depth limit is **64**.
You can increase this limit via `ConfigOptions`.

## Substitution Depth Limit

Substitution resolution has its own depth limit to avoid infinite recursion or stack overflows.
This is usually not an issue unless your configuration comes from untrusted user input.

## About substitution

Substitutions are resolved as the last step in parsing.
This means a substitution can refer to values defined later in the file or even in other included files.

⚠️ Avoid writing overly complex substitutions unless you fully understand the implementation details.
Behavior may differ slightly from the official Java version.

For example:

**substitution1.conf**

```hocon
a = hello

a = ${a}

a = ${b}

b = [1, 2]

b += ${a}

a = {}
```

**substitution2.conf**

```hocon
b = hello

b = ${b}

b = ${a}

a = [1, 2]

a += ${b}

b = {}
```

In Java’s implementation:

- `substitution1.conf` → `{"a":{},"b":[1,2,"hello"]}`

- `substitution2.conf` → `{"a":[1,2,{}],"b":{}}`

In the Rust implementation, both examples produce a parse error.
For clarity and consistency, avoid such tricky substitution patterns.

## Includes without File Extension

According to the HOCON spec, if a file extension is omitted, the loader attempts to parse all supported formats at the
given path (`JSON`, `JavaProperties`, `HOCON`) and merge them into a single object.

You can customize merge priority using the comparison function defined in `ConfigOptions`.

# Specification Coverage

- [x] Comments
- [x] Root braces may be omitted
- [x] Flexible key-value separators (`=` `:` `+=`)
- [x] Optional commas
- [x] Whitespace handling
- [x] Duplicate keys and object merging
- [x] Unquoted strings
- [x] Multi-line strings
- [x] Value concatenation
  - [x] String concatenation
  - [x] Array and object concatenation
- [x] Path expressions
- [x] Paths as keys
- [x] Substitutions
- [x] Includes
- [x] Conversion of numerically indexed objects into arrays
- [x] Duration unit format
- [ ] Period unit format
- [x] Size unit format

## Roadmap

- [ ] More descriptive error messages
- [ ] Serialize to HOCON format
- [ ] Serialize raw HOCON text
- [ ] Parse and preserve comments
- [ ] Refactor recursive functions to iterative implementations

# Documentation

For detailed API documentation, see [docs.rs](https://docs.rs/hocon-rs/latest/hocon_rs/)

# References

- Original Java implementation: [lightbend/confg](https://github.com/lightbend/config)
- HOCON format specification: [HOCON](https://github.com/lightbend/config/blob/main/HOCON.md)