#!/usr/bin/env bash

# Install Rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
echo "source $HOME/.cargo/env" >> ~/.bashrc

# Install Tockloader
pip3 install -U --user tockloader
echo "PATH=$HOME/.local/bin:$PATH" >> ~/.bashrc
$HOME/.local/bin/register-python-argcomplete tockloader >> ~/.bashrc
