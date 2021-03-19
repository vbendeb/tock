#!/usr/bin/env bash

# Build various things so that all of the setup work is already done.

# Make sure rustup is in the path. Not sure why I have to do this manually here.
source $HOME/.cargo/env

# Build one Cortex-M board.
pushd tock/boards/hail
make
popd

# Build one nRF52 board.
pushd tock/boards/nano33ble
make
popd

# Build one RISC-V board.
pushd tock/boards/earlgrey-nexysvideo
make
popd
