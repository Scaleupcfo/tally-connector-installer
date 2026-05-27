# Lekha Tally Agent

Local Windows agent that bridges the Lekha AI web app to Tally Prime on a user's PC.

## The problem

Tally Prime is desktop accounting software. Its data sits inside Tally on a user's PC; the only programmatic access is an XML-over-HTTP gateway on `localhost:9000`. Browsers cannot call `http://localhost:9000` directly from an HTTPS page — mixed-content blocking. So the Lekha AI website (HTTPS) cannot talk to Tally without a local helper.

## The fix

This installer drops a small Rust binary (`lekha_tally.exe`) into the user's `AppData`. It runs in the system tray and exposes an HTTPS server on `https://127.0.0.1:9100`. The Lekha AI web app calls *it*; it forwards to Tally; it returns clean JSON.

```
   ┌────────────────────────────────┐         ┌─────────────────────────────┐
   │  Lekha AI website              │         │  User's Windows PC          │
   │  (https://lekha.ai)            │         │  ┌───────────────────────┐  │
   │                                │   HTTPS │  │ Lekha Tally Agent     │  │
   │  browser JS:                   │ ──────▶ │  │ (this repo)           │  │
   │  fetch('https://127.0.0.1:9100│  bearer │  │ axum on :9100, tray   │  │
   │    /vouchers?company=...')    │  token  │  └──────────┬────────────┘  │
   └────────────────────────────────┘         │             │ XML/HTTP      │
                                              │             ▼               │
                                              │      ┌───────────────┐      │
                                              │      │ Tally Prime   │      │
                                              │      │ on :9000      │      │
                                              │      └───────────────┘      │
                                              └─────────────────────────────┘
```

## Endpoints

| Method | Path | Auth | What it returns |
|---|---|---|---|
| GET | `/health` | none | Liveness probe |
| GET | `/companies` | Bearer token | All companies loaded in Tally, each with its books period |
| GET | `/ledgers?company=<name>` | Bearer token | Master ledger list for one company |
| GET | `/vouchers?company=<name>&from=<iso>&to=<iso>` | Bearer token | Vouchers for one company in a date range |

All responses are JSON. Error shape: `{ "ok": false, "error": "..." }` with appropriate 4xx/5xx status.

See [TESTING.md](TESTING.md) for installation and end-to-end testing on a PC with real Tally.

## Security model

| Layer | What it blocks |
|---|---|
| `localhost`-only bind | Nothing on the network can reach the agent |
| Self-signed TLS | Encrypts the localhost wire; browsers initially warn (cert trust is a future enhancement) |
| CORS origin pin | Only `lekha.ai` (and dev origins) can read responses from a browser |
| Bearer pairing token | Even on the right origin, a request without the token gets 401. Token is a UUID generated on first run, stored at `%LOCALAPPDATA%\LekhaTallyInstaller\token.txt` |
| Constant-time token compare | Resists timing attacks |

## Repo layout

```
src/
  main.rs              — orchestration (spawn agent thread, run tray loop)
  agent.rs             — the HTTPS axum server (TLS, CORS, auth, handlers)
  tray.rs              — Windows tray icon + right-click menu
  auth.rs              — pairing token generation + constant-time compare
  tls.rs               — self-signed cert generation
  tally.rs             — module root (re-exports)
  tally/
    client.rs          — HTTP transport to Tally
    sanitize.rs        — strip Tally's malformed control chars
    dates.rs           — YYYYMMDD <-> YYYY-MM-DD
    companies.rs       — list loaded companies
    ledgers.rs         — list master ledgers
    vouchers.rs        — list vouchers in a date range

installer/
  lekha_tally_agent.iss   — Inno Setup script
installer_out/
  LekhaTallyAgentSetup.exe  — the built installer (generated, not checked in)
```

## Build from source

Prerequisites:
- Rust toolchain (`rustup`, `cargo` ≥ 1.95)
- MSVC build tools (auto-prompted by rustup on Windows)

Dev workflow:

```powershell
# Build + run as a console app, prints to stdout
cargo run

# Run the unit tests (the parser tests are real coverage)
cargo test

# Build the optimized release binary
cargo build --release
# -> target\release\lekha_tally.exe
```

## Build the installer

Prerequisites: **Inno Setup 6** (`winget install JRSoftware.InnoSetup`, no admin needed).

```powershell
cargo build --release
& "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe" installer\lekha_tally_agent.iss
# -> installer_out\LekhaTallyAgentSetup.exe
```

The installer is ~4 MB. It installs per-user (no UAC prompt), creates a Start Menu shortcut, sets auto-start on login, and registers an uninstaller in Settings → Apps.

## What's intentionally NOT done yet

- **Cert trust**: the self-signed TLS cert isn't added to the Windows trust store. Browsers warn on first hit. A future installer can call `certutil -addstore Root` as a custom action (admin required).
- **Voucher detail**: `Phase 6c` work to add bank allocations, inventory entries, dispatch details, GST tax-summary, batch allocations. The Python prototype at `Scaleupcfoai/tally-integration` has the parser logic for all of these — port when there's a real Lekha AI consumer asking for them.
- **Auto-update**: the installer doesn't self-update. Users will need to re-download for new versions. Add a check-for-updates mechanism (or just let the Lekha AI website warn if `/health` returns a stale version).
