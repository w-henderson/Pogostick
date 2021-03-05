#!/bin/bash

echo "creating image"
qemu-img create disk.img 32M
echo "copying kernel"
dd conv=notrunc if=target/pogostick/debug/bootimage-pogostick.bin of=disk.img
echo "starting emulator"

if [[ -z "${QEMU_PATH}" ]]; then
  qemu-system-x86_64 -hdc "disk.img"
else
  qemu-system-x86_64 -L "${QEMU_PATH}" -hdc "disk.img"
fi