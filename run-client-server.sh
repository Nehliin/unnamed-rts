#!/bin/bash
echo "Starting server"
cargo run --bin server &
sleep 3s
echo "Starting client"
cargo run --bin client