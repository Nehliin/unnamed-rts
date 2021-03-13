Write-host "Starting server"
Start-Process cargo -ArgumentList "run", "--bin", "server"
Start-Sleep -Seconds 3
Write-host "Starting client"
cargo run --bin client