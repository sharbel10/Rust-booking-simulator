# Part C — Tauri boundary for the booking simulator

**Proposal.** Build one Arabic/English screen for the AED 1,000 videography booking from Part B. Simulate the clock and escrow provider. Demonstrate a rejected start before funding, approval of the raw footage without payout, renewal at hour 49, and completion after confirmed payout.

## Engineering decisions

- **Leptos in the WebView.** Write the UI in Rust and compile it to WebAssembly with Trunk. Use HTML and CSS for layout, set `lang` and `dir` when switching languages, and translate error codes in the UI. Leptos gives us a documented Tauri integration and a Rust UI without authoring TypeScript. Svelte could reuse more of the existing frontend. For this small simulator, sharing Rust tooling is worth the WebAssembly build and IPC setup. [Tauri’s Leptos guide](https://v2.tauri.app/start/frontend/leptos/)
- **Tauri as the native client.** The WebView calls specific commands such as `approve_milestone` through `invoke`, with a milestone ID and delivery ID. Native Rust forwards the request to the API and returns display data or a structured error. Expose commands for user actions. Keep provider confirmations inside the API’s mock adapter. [Tauri commands](https://v2.tauri.app/develop/calling-rust/)
- **Axum and a shared Rust domain.** `booking-core` owns pure transition rules and accepts an explicit clock. Axum owns booking mutations, storage, and the renewal worker. Start it as a separate localhost process for the demo so the worker can continue when the Tauri window closes. At hour 49 the same rules apply. The hold deadline determines when renewal runs. [Axum](https://docs.rs/axum/latest/axum/)
- **PostgreSQL behind the API.** Use a dedicated local `booking_demo` database through SQLx. Store `bookings`, `milestones` (including delivery version and any revision reason), `escrow_holds` (status, amount in minor units, currency, and UTC expiry), and `escrow_operations` (type, status and unique operation key). Persist state changes and pending operations together when they arise from one transition. Deduplicate renewal by hold and prior expiry, and record the new expiry only after the mock provider confirms it. Only Axum receives the database connection string. [SQLx connection pools](https://docs.rs/sqlx/latest/sqlx/pool/index.html)

## Implementation order

1. Create an isolated Cargo workspace for the simulator. Implement `booking-core` and test valid and invalid transitions using a fake clock.
2. Connect one Leptos screen to a Tauri command and an Axum endpoint. Use in-memory storage for the first end-to-end check.
3. Add PostgreSQL migrations, SQLx persistence and the mock escrow worker. Restart the API and confirm that state persists. Retry the same renewal and confirm that only one operation is recorded.
4. Prepare the interview demo. Switch Arabic/English and verify RTL/LTR. Show the failed start, both milestones, and the hour-49 renewal. Close Tauri while the API is running, then reopen it and read the persisted result.

The interview simulation uses synthetic records and a mock escrow adapter. A live marketplace would need authenticated participants, verified provider events and a funded escrow integration. Dispute adjudication stays a future extension.

## How I use AI

I define the acceptance scenarios and approve business rules. The project skill gives the implementation agent the agreed boundaries. A separate reviewer checks state transitions, retries, and where data is trusted. I run the checks, examine the diff, and keep a short record of which suggestions I accepted or rejected and why.
