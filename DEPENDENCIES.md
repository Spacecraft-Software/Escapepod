<!--
SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
SPDX-License-Identifier: CC-BY-SA-4.0
-->
<!-- GFM Document
     title:      Escape Pod — Dependencies
     author:     Mohamed Hammad & Spacecraft Software
     date:       2026-11-01
     license:    CC-BY-SA-4.0
     project:    Escape Pod
-->

# Escape Pod — Dependencies

The Standard §24.1 qualification record. Category B records the dependencies in the **trust
path**: anything that parses untrusted input, touches devices or the filesystem with privilege,
or sits on the keystroke path. Escape Pod's whole runtime graph is small enough that every direct
runtime dependency is recorded. Test-only crates (`proptest`, `assert_cmd`) are outside the trust
path and rely on the §24.2 tooling (`cargo audit`, `cargo deny`) alone.

Qualified at gate G2 on 2026-11-01. Advisory history is from the RustSec database as fetched that
day. Unsafe counts are occurrences of the `unsafe` keyword under each crate's `src/`, a rough
stand-in for `cargo geiger`, which is not packaged for the reference machine. The full runtime
graph is 49 crates with no duplicate versions (`cargo tree -d -e normal` is empty).

`escapepod-core`, the engine, has **no dependencies**: it is `std` only.

## evdev 0.13.2

| Field | Record |
|---|---|
| Purpose | Opens `/dev/input/event*`, reads events, takes the exclusive grab (`EVIOCGRAB`), reads key state, and creates the uinput virtual keyboard. Without it: the ioctl definitions and the uinput setup sequence, written by hand in `unsafe` code |
| Provenance | <https://github.com/cmr/evdev>, community maintained; Apache-2.0 OR MIT |
| Maintenance | 0.13.2 released 2025-09-15, the newest; 22 releases since 2015; active issue tracker |
| Security history | No RustSec advisory |
| Unsafe posture | 69 occurrences, all ioctl and `input_event` FFI. Load-bearing: a wrong ioctl could misreport a grab. Mitigated by the uinput-loopback hardware tests (P-030) |
| Transitive weight | 10 crates (`bitvec` and its four helpers, `bitflags`, `cfg-if`, `libc`, `memoffset`, `nix` 0.29) |
| Alternatives considered | `input-linux` (lower level, more of our own unsafe-free plumbing but no maintained uinput builder); `evdev-rs` (binds the C libevdev, a C dependency) |
| Exit plan | Vendor it under §4.2; the crate is 5 700 lines, of which the parts used (open, grab, fetch, key state, uinput builder) are a fraction |

## nix 0.29.0

| Field | Record |
|---|---|
| Purpose | Safe wrappers for `poll(2)`, `signalfd(2)`, `sigprocmask(2)`, `inotify(7)` (hotplug), and group lookup for the status socket. Without it: these syscalls in `unsafe` `libc` calls |
| Provenance | <https://github.com/nix-rust/nix>, the nix-rust organisation; MIT |
| Maintenance | 0.29.0 released 2024-05-24; newest is 0.31.3 (2026-05-11). 0.29 is taken because `evdev` 0.13.2 already links it, so the graph keeps one copy |
| Security history | RUSTSEC-2021-0119 (out-of-bounds write in `getgrouplist`), fixed in 0.23.0 and handled by a prompt point release on every maintained branch. Escape Pod never calls `getgrouplist` |
| Unsafe posture | 825 occurrences: it is an FFI layer by design. Load-bearing for the event loop and the signal path |
| Transitive weight | 4 crates (`bitflags`, `cfg-if`, `libc`, `memoffset`) |
| Alternatives considered | `rustix` (also sound, but a second syscall layer next to the `nix` that `evdev` brings); `signal-hook` for signals (adds a crate, and a self-pipe where `signalfd` needs none) |
| Exit plan | Move to `nix` 0.31 together with `evdev`'s next release; failing that, `rustix` covers every call used |

## x11rb 0.14.0

| Field | Record |
|---|---|
| Purpose | The X11 fallback: a pure-Rust X connection, the RECORD extension to observe keys and the XTEST extension to inject the tap key. Without it: the X11 wire protocol by hand, or the C libX11 |
| Provenance | <https://github.com/psychon/x11rb>, maintained by its author with regular contributors; MIT OR Apache-2.0 |
| Maintenance | 0.14.0 released 2026-07-16, the newest; 20 releases since 2019 |
| Security history | No RustSec advisory under `x11rb` or `x11rb-protocol` |
| Unsafe posture | 75 occurrences in `x11rb`, none in `x11rb-protocol`, which parses the server's replies. The `allow-unsafe-code` feature (the libxcb FFI) is off, so the pure-Rust connection is used |
| Transitive weight | 6 crates (`gethostname`, `rustix`, `x11rb-protocol`, …) |
| Alternatives considered | `xcb` (binds the C libxcb); the `x11` crate (binds libX11, unsafe everywhere) |
| Exit plan | Vendor it; only the RECORD and XTEST requests and the core event parser are used |

## clap 4.6.7

| Field | Record |
|---|---|
| Purpose | Parses the command line (untrusted argv): the command tree, global flags and `--help`. Without it: a hand-written parser and help renderer |
| Provenance | <https://github.com/clap-rs/clap>, the clap-rs organisation; MIT OR Apache-2.0 |
| Maintenance | 4.6.7 released 2026-09-14; very active, 462 releases |
| Security history | No RustSec advisory |
| Unsafe posture | 5 occurrences in `clap_builder`, none on argument parsing of untrusted strings |
| Transitive weight | 11 crates, default features off (no colour, no suggestions) |
| Alternatives considered | `lexopt` and `pico-args` (smaller, but no derive, no generated help, no completion support) |
| Exit plan | Replace with `lexopt` and a hand-written help text; the tree has seven commands |

## serde 1.0.229, toml 1.1.8, serde_json 1.0.154

| Field | Record |
|---|---|
| Purpose | `toml` parses the configuration file (untrusted input) into typed structs through `serde`, with spans for the line and column of an error (ESC-SRS-092). `serde_json` writes the JSON envelope, errors and status-socket frames. Without them: a TOML parser and a JSON writer by hand |
| Provenance | <https://github.com/serde-rs/serde>, <https://github.com/serde-rs/json>, <https://github.com/toml-rs/toml>; all MIT OR Apache-2.0 |
| Maintenance | All released within the last four months; the most depended-on crates in the ecosystem |
| Security history | No RustSec advisory for any of the three |
| Unsafe posture | 2 (`serde`), 16 (`serde_json`, number formatting), 3 (`toml`). The configuration parser is fuzzed at M7 (ESC-SRS-081) |
| Transitive weight | 8 for `serde` with derive, 4 more for `serde_json`, 5 more for `toml` (default features off: parse and serde only) |
| Alternatives considered | `basic-toml` (no spans, so no line and column for unknown keys) |
| Exit plan | None needed for `serde`; `toml` could be replaced by `toml_edit`'s parser from the same project |

## jiff 0.2.38

| Field | Record |
|---|---|
| Purpose | Formats the ISO 8601 UTC timestamps in the JSON envelope, errors and the status socket (Standard §14). Without it: a civil-date conversion from `SystemTime` by hand |
| Provenance | <https://github.com/BurntSushi/jiff>, by the author of `regex` and `ripgrep`; Unlicense OR MIT |
| Maintenance | 0.2.38 released 2026-10-06; frequent releases |
| Security history | No RustSec advisory |
| Unsafe posture | 79 occurrences, mostly in the time-zone database code, which is off (`default-features = false`, `std` only); only `Timestamp` is used |
| Transitive weight | 1 crate |
| Alternatives considered | `chrono` (heavier, and its local-time types invite §14 violations); `time` (also sound, more features than needed) |
| Exit plan | A 30-line UTC formatter from `SystemTime`; nothing else of `jiff` is used |

---

*— Built by [Spacecraft Software](https://SpacecraftSoftware.org/) —*
