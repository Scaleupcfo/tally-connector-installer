# Lekha AI Tally Connector

Local Windows CORS proxy that bridges the Lekha AI web app to Tally Prime on a user's PC.

## The problem

Tally Prime is desktop accounting software. Its data sits inside Tally on a user's PC; the only programmatic access is an XML-over-HTTP gateway on `localhost:9000`. Browsers cannot call `http://localhost:9000` directly from an HTTPS page — mixed-content blocking. So the Lekha AI website (HTTPS) cannot talk to Tally without a local helper.

## The fix

This connector drops a small Rust binary (`lekha_tally.exe`) into the user's `AppData`. It runs in the system tray and exposes an HTTPS server on `https://127.0.0.1:9100`. The Lekha AI web app POSTs raw XML to it; it forwards to Tally; it returns Tally's XML response (sanitized of invalid chars).

The connector is a **dumb pipe** — it never inspects, parses, or modifies the XML content. All Tally-specific logic (Export queries, Import vouchers, field selection) lives in the web app, not the installed binary.

```
   ┌────────────────────────────────┐         ┌─────────────────────────────┐
   │  Lekha AI website              │         │  User's Windows PC          │
   │  (https://lekhaai.app)         │         │  ┌───────────────────────┐  │
   │                                │   HTTPS │  │ Lekha AI Tally        │  │
   │  browser JS:                   │ ──────▶ │  │ Connector (this repo) │  │
   │  fetch('https://127.0.0.1:9100│  bearer │  │ axum on :9100, tray   │  │
   │    /tally', {method:'POST',   │  token  │  └──────────┬────────────┘  │
   │    body: xmlEnvelope})        │         │             │ XML/HTTP      │
   └────────────────────────────────┘         │             ▼               │
                                              │      ┌───────────────┐      │
                                              │      │ Tally Prime   │      │
                                              │      │ on :9000      │      │
                                              │      └───────────────┘      │
                                              └─────────────────────────────┘
```

## Endpoints

| Method | Path | Auth | What it does |
|---|---|---|---|
| GET | `/health` | none | Liveness probe |
| POST | `/tally` | Bearer token | Forwards XML body to Tally on :9000, returns XML response |

The `/tally` endpoint accepts any Tally XML — Export (read) or Import (write). Error responses are JSON: `{ "ok": false, "error": "..." }` with appropriate 4xx/5xx status.

See [TESTING.md](TESTING.md) for installation and end-to-end testing on a PC with real Tally.

## Security model

| Layer | What it blocks |
|---|---|
| `localhost`-only bind | Nothing on the network can reach the connector |
| Self-signed TLS | Encrypts the localhost wire; browsers initially warn (cert trust is a future enhancement) |
| CORS origin pin | Only `lekhaai.app`, `lekha.ai` (and dev origins) can read responses from a browser |
| Bearer pairing token | Even on the right origin, a request without the token gets 401. Token is a UUID generated on first run, stored at `%LOCALAPPDATA%\LekhaAI\TallyConnector\token.txt` |
| Constant-time token compare | Resists timing attacks |
| Content-Type validation | Only `text/xml` or `application/xml` accepted on `/tally` |
| 10 MB body limit | Prevents oversized request abuse |

## Repo layout

```
src/
  main.rs              — orchestration (spawn agent thread, run tray loop)
  agent.rs             — the HTTPS axum server (TLS, CORS, auth, passthrough handler)
  tray.rs              — Windows tray icon + right-click menu (Lekha AI branded)
  auth.rs              — pairing token generation + constant-time compare
  tls.rs               — self-signed cert generation
  tally.rs             — module root (re-exports)
  tally/
    client.rs          — HTTP transport to Tally (forward_xml)
    sanitize.rs        — strip Tally's malformed control chars

installer/
  lekha_tally_agent.iss   — Inno Setup script
test-page/
  index.html              — browser-based test UI with sample XML templates
```

## Build from source

Prerequisites:
- Rust toolchain (`rustup`, `cargo` >= 1.95)
- MSVC build tools (auto-prompted by rustup on Windows)

Dev workflow:

```powershell
# Build + run as a console app, prints to stdout
cargo run

# Run the unit tests
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

The installer is ~4 MB. It installs per-user (no UAC prompt), creates a Start Menu shortcut under "Lekha AI", sets auto-start on login, and registers an uninstaller in Settings -> Apps.

## What's intentionally NOT done yet

- **Cert trust**: the self-signed TLS cert isn't added to the Windows trust store. Browsers warn on first hit. A future installer can call `certutil -addstore Root` as a custom action (admin required).
- **Auto-update**: the connector doesn't self-update. Users will need to re-download for new versions. Add a check-for-updates mechanism (or let the Lekha AI website warn if `/health` returns a stale version).
