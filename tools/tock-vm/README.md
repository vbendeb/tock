Tock Virtual Machine Setup Tool
===============================

This folder contains files to support the creation of a virtual machine image
with all of the "getting started" steps finished. This VM image is often
convenient for new users to Tock who just want to be able to get started and
program a board quickly.

## Packer Tool

We use [Packer](https://www.packer.io/docs) to facilitate building the virtual
machine image.

## Building the Image

You will need:

1. Packer
2. VirtualBox

Then run:

    $ ./build.sh

This will download the Ubuntu image .iso, create a new VM with the .iso, and
then configure the VM according to the "tock-vm-ubuntu.json" configuration file.

At the beginning, Packer needs to "type" into the VM. So after you run the
script you cannot click anything until Packer prints "waiting for SSH". It's
only the beginning where this happens, so after the autoinstall tool gets to
work you don't need to keep the VM in the foreground.

If everything is successful you should have
`output-tock-vm-ubuntu-20.04-amd64/tock-vm-ubuntu-20.04-amd64.ova` which can
then be imported into VirtualBox to create a virtual machine.
