#!/bin/bash

echo "creating image"
qemu-img create disk.img 32M
echo "copying kernel"
dd conv=notrunc if=target/pogostick/debug/bootimage-pogostick.bin of=disk.img
echo "starting emulator"
qemu-system-x86_64 -L D:\\Programs\\qemu -hdc "disk.img"