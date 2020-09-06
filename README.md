# Rusty-xv6

[![Build](https://github.com/algon-320/rusty-xv6/workflows/Build/badge.svg)](https://github.com/algon-320/rusty-xv6/actions?query=workflow%3ABuild)
[![Test on QEMU](https://github.com/algon-320/rusty-xv6/workflows/Test%20on%20QEMU/badge.svg)](https://github.com/algon-320/rusty-xv6/actions?query=workflow%3A%22Test+on+QEMU%22)

Learn [xv6 OS (x86 version)](https://github.com/mit-pdos/xv6-public) through
re-implementing it in Rust (+ inline-assembly and some unstable features).

## Requirements
- Nightly Rust + cargo
- qemu-system-i386
- other build tools:
    - `make`
    - `objcopy`
    - `dd`

## Build and Run
```
$ make qemu
```

## Debug
```
$ make gdb
```
and on another terminal
```
$ make gdb-attach
```

## Test
```
$ make test
```