#!/bin/bash

set -e

# Run all instances
cd atomic_disc_drive

cargo build

tmp1=$(mktemp -d)
tmp2=$(mktemp -d)
tmp3=$(mktemp -d)

cargo run ../simple_config 1 "$tmp1" & 
cargo run ../simple_config 2 "$tmp2" &
cargo run ../simple_config 3 "$tmp3" &
cd ../

sleep 4

# Run simple write test
cd ./public-tests/
cargo test --test reconnect external_write -- --ignored
cd ../

kill %2
kill %3

# Rerun one instance, should still work
cd atomic_disc_drive
cargo run ../simple_config 2 "$tmp2" &
cd ../

cd ./public-tests/
cargo test --test reconnect external_read -- --ignored
[ ! $? ] && echo "FAIL"

kill %4
kill %1
