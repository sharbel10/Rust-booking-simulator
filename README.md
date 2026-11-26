# Booking simulator

A local interview demo for a two-milestone videography booking. The escrow provider, identities, and clock are simulated.

Run the terminal scenario from the workspace root:

```powershell
cargo run -p booking-core --example booking_demo
```

## Web

From the workspace root, build the UI, then start Axum:

```powershell
Push-Location desktop/ui
trunk build --release
Pop-Location
cargo run -p booking-server
```

Open http://127.0.0.1:3000 in a browser. Axum serves the compiled UI and booking API from the same address. Rebuild the UI after editing it.

## Desktop

Launch the desktop demo in two terminals:

```powershell
cargo run -p booking-server
```

```powershell
cd desktop
cargo tauri dev
```

Start the raw footage before funding to see the rejection. Then accept the booking, confirm funding, and start, submit, and approve the raw footage. Advance to hour 49 twice and check that the expiry moves from 50 to 74 only once. Complete the final video and confirm payout.

Leptos renders the shared UI. The browser sends HTTP requests to Axum. The desktop app calls a native Tauri command. Tauri forwards the request to the local Axum API on port 3000. The API applies the shared Rust booking rules. The time advance executes the renewal check on demand. Without DATABASE_URL the server runs in memory.

## PostgreSQL

From the workspace root, start the local database in Docker and tell the server to use it:

```powershell
docker compose up -d --wait
$env:DATABASE_URL = 'postgresql://booking:booking_demo_local@127.0.0.1:5433/booking_demo'
cargo run -p booking-server
```

The server applies the migrations in `server/migrations`. Restarting the server with the same URL restores the booking, milestones, simulated clock, and escrow state. The mock provider confirms renewal and payout locally, and the database records each confirmation in the same transaction as its state update. The **Reset demo** button clears the simulation history and starts a new run.
