# Testing Guide — Lekha Tally Agent

For: anyone with a Windows PC running Tally Prime.
Goal: install the agent, point a browser at it, see real Tally data come back as JSON.

You'll need:
- A Windows PC (10 or 11, 64-bit)
- Tally Prime installed and at least one company loaded
- 5 minutes

---

## 1. Install

1. Get `LekhaTallyAgentSetup.exe` (from the repo's releases page, or wherever the build was shared).
2. Double-click it.
3. **No UAC prompt should appear** — this is a per-user install, lands in `%LOCALAPPDATA%\Programs\Lekha\TallyAgent`.
4. On the last screen, keep both boxes ticked:
   - "Start Lekha Tally Agent automatically when I sign in"
   - "Launch Lekha Tally Agent now"
5. Click **Finish**.

**Verify it's running:** look at the system tray (bottom-right of taskbar, click `^` to expand if hidden). You should see a **blue square icon** with tooltip *"Lekha Tally Agent — local bridge to Tally Prime"*. Right-click it; you should see menu items for **Show pairing token**, **Open data folder**, **Quit**.

If you don't see the icon: open `%LOCALAPPDATA%\Programs\Lekha\TallyAgent\lekha_tally.exe` directly. If that fails too, paste the error.

---

## 2. Enable Tally's HTTP gateway

In Tally Prime:

1. Press **F1** → **Settings**
2. Go to **Connectivity** → **Client/Server configuration**
3. Set **TallyPrime is acting as** = **Both**
4. Set **Port** = **9000**
5. Save (Ctrl+A)
6. **Keep at least one company open in Tally** — the gateway only returns data for currently-loaded companies.

Quick sanity check from any PowerShell window:

```powershell
Test-NetConnection -ComputerName localhost -Port 9000
```

Should report `TcpTestSucceeded : True`. If False, the gateway isn't on — re-check the steps above.

---

## 3. Grab your pairing token

Right-click the tray icon → **Show pairing token**. Notepad opens with a UUID like:

```
54db5b26-324d-411d-af3e-08e11a7aec16
```

Copy it. This is the secret that proves "this browser session belongs to you" — every request to a protected endpoint must include it.

---

## 4. Test each endpoint

Open PowerShell. Paste your token into the `$TOKEN` line, then run the commands one at a time.

```powershell
$TOKEN = "PASTE-YOUR-TOKEN-HERE"
$AUTH = "Authorization: Bearer $TOKEN"
$BASE = "https://127.0.0.1:9100"
```

### 4a. Health check (no auth required)

```powershell
curl.exe -sk -w "`nStatus: %{http_code}`n" $BASE/health
```

**Expected:** Status 200, body `{"ok":true,"service":"lekha_tally_installer","version":"0.1.0"}`.

### 4b. List loaded companies

```powershell
curl.exe -sk -H $AUTH -w "`nStatus: %{http_code}`n" $BASE/companies
```

**Expected:** Status 200, body looks like:
```json
{"ok":true,"companies":[
  {"name":"YOUR COMPANY","books_start":"2023-04-01","books_end":"2025-03-31"}
]}
```

If you see `"companies":[]` — Tally is reachable but no company is loaded. Open one in Tally and re-try.

### 4c. List master ledgers for one company

Use the exact `name` from the previous response (case- and space-sensitive). URL-encode spaces as `%20` or `+`.

```powershell
$COMPANY = "YOUR%20COMPANY%20NAME"
curl.exe -sk -H $AUTH -w "`nStatus: %{http_code}`n" "$BASE/ledgers?company=$COMPANY"
```

**Expected:** Status 200, list of ledger objects with `name`, `parent_group`, `opening_balance`, `closing_balance`, `party_gstin`, etc.

### 4d. List vouchers for a date range

```powershell
curl.exe -sk -H $AUTH -w "`nStatus: %{http_code}`n" "$BASE/vouchers?company=$COMPANY&from=2024-04-01&to=2025-03-31"
```

**Expected:** Status 200, list of voucher objects with `date`, `voucher_type`, `voucher_number`, `party_ledger_name`, `ledger_entries`, etc.

Adjust the dates to fall inside your company's books period (visible from step 4b's response).

---

## 5. Common errors and what they mean

| Status | Body | What's wrong | Fix |
|---|---|---|---|
| 401 | `missing or invalid Authorization: Bearer <token>` | Token not sent, or wrong | Re-copy from tray menu |
| 503 | `Tally is not reachable on port 9000` | Tally isn't running, or port 9000 isn't enabled | See section 2 |
| 502 | `Tally returned malformed XML: ...` | Tally responded with corrupt XML | Restart Tally and try again |
| 400 | `invalid from date: ...` | Date isn't `YYYY-MM-DD` | Use ISO format only |
| 400 | `company name is required` | `company` query param empty | Pass the exact name from `/companies` |
| 200 | `"companies":[]` | Tally is up, but no company is open | Open a company in Tally |

---

## 6. Test from a real browser (optional)

The Lekha AI website will call the agent from inside a browser tab. Until the cert is trusted system-wide (a future installer enhancement), the first browser hit will show a **"Your connection is not private"** warning.

To accept it for testing in Chrome/Edge:
1. Open `https://127.0.0.1:9100/health` directly in the browser
2. Click **Advanced** → **Proceed to 127.0.0.1 (unsafe)**
3. After that, the cert is remembered for that browser; subsequent calls from `https://lekha.ai` to the agent will work without warning.

---

## 7. Uninstall

Windows **Settings → Apps → Installed apps → Lekha Tally Agent → Uninstall**.

This removes the binary, the auto-start entry, and the Start Menu shortcut. **It does NOT delete the per-user data folder** at `%LOCALAPPDATA%\LekhaTallyInstaller\` (cert, key, pairing token). Delete that manually if you want a truly clean wipe.

---

## What to report back

If you can complete steps 4a–4d with real data from your books, the agent works end-to-end. Please share:

1. Output of step 4b (`/companies`) — confirms multi-company discovery.
2. A trimmed sample of step 4d (`/vouchers`) — confirms voucher + ledger-entry parsing.
3. Any voucher fields that **should be in our output but aren't** — current Phase 6b ships only headers + ledger entries. Bank allocations, inventory, GST tax-summary, and dispatch details are deliberately deferred to a future "Phase 6c" pending real-data review.
