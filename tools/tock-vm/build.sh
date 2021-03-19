#!/usr/bin/env bash

sed '/^[[:blank:]]*#/d;s/#.*//' tock-vm-ubuntu.json > tock-vm-ubuntu-no-comments.json

packer build tock-vm-ubuntu-no-comments.json
