---
name: booking-simulator
description: Implements or extends the Part C Rust booking simulator using the agreed Leptos, Tauri, Axum, and PostgreSQL boundaries. Use when explicitly invoked for simulator work.
disable-model-invocation: true
---

# Booking simulator

Read `docs/part-c-engineering-plan.md` before editing. Work inside this simulator workspace. Implement one file at a time, explain it to the user, and wait for the user to continue before the next file. Implement the smallest end-to-end step that demonstrates the requested behavior.

1. Trace the event from Leptos through Tauri to Axum. Keep authorization and persistence in the server, transition rules in `booking-core`, and localization in the UI.
2. Use synthetic fixtures and a fake clock. Producer events come from the mock escrow adapter. Treat confirmation, user approval, and payout as different events.
3. Preserve rejection reasons with delivery IDs. Use a stable key for escrow retries. Make decisions at hour 49 from hold expiry and booking state.
4. Report the changed files, the exact checks run and their outcomes, and assumptions that need a product decision. Explain the implementation so the interview candidate can trace it independently.
