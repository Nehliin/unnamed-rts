Write-host "Starting server"
Start-Process cargo -ArgumentList "run", "--bin", "server"
Start-Sleep -Seconds 1
Write-host "Starting client"
$env:RUST_BACKTRACE = 1
cargo run --bin client