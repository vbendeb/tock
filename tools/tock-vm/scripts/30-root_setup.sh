#!/usr/bin/env bash

# All setup that requires root.

# Enable known tock boards to be properly recognized.
echo 'ATTRS{idVendor}=="0403", ATTRS{idProduct}=="6015", MODE="0666"' > /etc/udev/rules.d/99-ftdi.rules
echo 'ATTRS{idVendor}=="2341", ATTRS{idProduct}=="005a", MODE="0666"' > /etc/udev/rules.d/98-arduino.rules
echo 'SUBSYSTEMS=="usb", ATTRS{idVendor}=="0483", ATTRS{idProduct}=="374b", MODE:="0660", GROUP="dialout", SYMLINK+="stlinkv2-1_%n"' > /etc/udev/rules.d/97-stlinkv2-1.rules
