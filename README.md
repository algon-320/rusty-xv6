# Rusty-xv6

Learn [xv6 OS (x86 version)](https://github.com/mit-pdos/xv6-public) through
re-implementing it in Rust (+ inline-assembly and some unstable features).

## Requirements
- Nightly Rust + cargo
- qemu-system-i386
- xv6 filesystem image (currently)
    1. build [xv6](https://github.com/mit-pdos/xv6-public).
    2. copy `fs.img` here.
- other build tools:
    - `make`
    - `objcopy`
    - `dd`

## Build and Run
```
$ make qemu
```

## Debug
Set `GDB_EXTERN_TERM` in Makefile to open GDB on your terminal.
(Default: `gnome-terminal`)
```
$ make gdb
```
and on another terminal
```
$ make gdb-attach
```

## Test
```
$ make test PROFILE=debug
```
`PROFILE=debug` is currently required.
(TODO: It should be testable in any build modes.)