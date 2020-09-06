#!/bin/bash
SCRIPT_DIR=$(cd $(dirname $0); pwd)
if [[ $# -eq 0 ]]; then
    echo "Usage: $0 [kernel-bin]"
    exit 1
fi
kernel_bin=$(cd $(dirname $1) && pwd)/$(basename $1)
if [[ ! -f "${kernel_bin}" ]]; then
    echo "${kernel_bin} doesn't exist"
    exit 1
fi
make -C ${SCRIPT_DIR} -s qemu-limited -W "${kernel_bin}" KERNEL_BIN="${kernel_bin}"