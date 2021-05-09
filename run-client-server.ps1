Write-host "Starting server"
Start-Process cargo -ArgumentList "run", "--bin", "server"
Write-host "Starting client"
$env:RUST_BACKTRACE = 1
cargo run --bin client