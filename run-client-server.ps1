﻿Write-host "Starting server"
Start-Process cargo -ArgumentList "run", "--bin", "server", "--no-default-features"
Start-Sleep -Seconds 1
Write-host "Starting client"
$env:RUST_BACKTRACE = 1
$env:RUST_LOG = "wgpu_core=warn"
cargo run --bin client 