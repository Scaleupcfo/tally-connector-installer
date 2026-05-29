# Testing Guide — Lekha AI Tally Connector

> **Note:** this guide is for *validating* that the connector works end-to-end against a real Tally — not how end users will eventually use it. Once Lekha AI's website integrates the pairing + sync flow, real users only do step 1 (install) and step 2 (enable Tally's port); everything from step 3 onward happens inside the browser automatically.

For: anyone with a Windows PC running Tally Prime.
Goal: install the connector, POST raw XML to Tally via the CORS proxy, see real Tally data come back.

You'll need:
- A Windows PC (10 or 11, 64-bit)
- Tally Prime installed and at least one company loaded
- 5 minutes

---

## 1. Install

1. In the repo, open the `dist/` folder. Download `LekhaTallyAgentSetup.exe`.
2. Double-click it.
3. **No UAC prompt should appear** — this is a per-user install, lands in `%LOCALAPPDATA%\Programs\LekhaAI\TallyConnector`.
4. On the last screen, keep both boxes ticked:
   - "Start Lekha AI Tally Connector automatically when I sign in"
   - "Launch Lekha AI Tally Connector now"
5. Click **Finish**.

**Verify it's running:** look at the system tray (bottom-right of taskbar, click `^` to expand if hidden). You should see a **lime-green "L" icon** with tooltip *"Lekha AI — Tally Connector"*. Right-click it; you should see menu items for **Show pairing token**, **Open data folder**, **Quit**.

---

## 2. Enable Tally's HTTP gateway

In Tally Prime:

1. Press **F1** -> **Settings**
2. Go to **Connectivity** -> **Client/Server configuration**
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

Right-click the tray icon -> **Show pairing token**. Notepad opens with a UUID like:

```
54db5b26-324d-411d-af3e-08e11a7aec16
```

Copy it. This is the secret that proves "this browser session belongs to you" — every request to a protected endpoint must include it.

---

## 4. Test the endpoints

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

**Expected:** Status 200, body `{"ok":true,"service":"lekha_tally_proxy","version":"0.2.0"}`.

### 4b. List loaded companies (Export)

Save this as `list_companies.xml`:

```xml
<ENVELOPE>
  <HEADER>
    <VERSION>1</VERSION>
    <TALLYREQUEST>Export</TALLYREQUEST>
    <TYPE>Collection</TYPE>
    <ID>ListOfLoadedCompanies</ID>
  </HEADER>
  <BODY>
    <DESC>
      <STATICVARIABLES>
        <SVEXPORTFORMAT>$$SysName:XML</SVEXPORTFORMAT>
      </STATICVARIABLES>
      <TDL>
        <TDLMESSAGE>
          <COLLECTION NAME="ListOfLoadedCompanies" ISINITIALIZE="Yes">
            <TYPE>Company</TYPE>
            <FETCH>Name, StartingFrom, EndingAt</FETCH>
          </COLLECTION>
        </TDLMESSAGE>
      </TDL>
    </DESC>
  </BODY>
</ENVELOPE>
```

```powershell
curl.exe -sk -X POST -H $AUTH -H "Content-Type: text/xml" --data-binary "@list_companies.xml" -w "`nStatus: %{http_code}`n" $BASE/tally
```

**Expected:** Status 200, Tally XML response containing `<COMPANY>` elements.

### 4c. List ledgers for a company (Export)

Replace `YOUR_COMPANY_NAME` with the exact name from 4b's response:

```xml
<ENVELOPE>
  <HEADER>
    <VERSION>1</VERSION>
    <TALLYREQUEST>Export</TALLYREQUEST>
    <TYPE>Collection</TYPE>
    <ID>LedgerMasters</ID>
  </HEADER>
  <BODY>
    <DESC>
      <STATICVARIABLES>
        <SVCURRENTCOMPANY>YOUR_COMPANY_NAME</SVCURRENTCOMPANY>
        <SVEXPORTFORMAT>$$SysName:XML</SVEXPORTFORMAT>
      </STATICVARIABLES>
      <TDL>
        <TDLMESSAGE>
          <COLLECTION NAME="LedgerMasters" ISINITIALIZE="Yes">
            <TYPE>Ledger</TYPE>
            <FETCH>NAME, PARENT, OPENINGBALANCE, CLOSINGBALANCE</FETCH>
          </COLLECTION>
        </TDLMESSAGE>
      </TDL>
    </DESC>
  </BODY>
</ENVELOPE>
```

### 4d. Create a test voucher (Import)

**Warning:** This writes data to Tally. Use a test company.

```xml
<ENVELOPE>
  <HEADER>
    <VERSION>1</VERSION>
    <TALLYREQUEST>Import</TALLYREQUEST>
    <TYPE>Data</TYPE>
    <ID>All Masters</ID>
  </HEADER>
  <BODY>
    <DESC>
      <STATICVARIABLES>
        <SVCURRENTCOMPANY>YOUR_COMPANY_NAME</SVCURRENTCOMPANY>
      </STATICVARIABLES>
    </DESC>
    <DATA>
      <TALLYMESSAGE xmlns:UDF="TallyUDF">
        <VOUCHER VCHTYPE="Sales" ACTION="Create">
          <DATE>20260530</DATE>
          <NARRATION>Test voucher via Lekha AI</NARRATION>
          <VOUCHERTYPENAME>Sales</VOUCHERTYPENAME>
          <PARTYLEDGERNAME>Cash</PARTYLEDGERNAME>
          <ALLLEDGERENTRIES.LIST>
            <LEDGERNAME>Cash</LEDGERNAME>
            <ISDEEMEDPOSITIVE>Yes</ISDEEMEDPOSITIVE>
            <AMOUNT>-1000.00</AMOUNT>
          </ALLLEDGERENTRIES.LIST>
          <ALLLEDGERENTRIES.LIST>
            <LEDGERNAME>Sales Account</LEDGERNAME>
            <ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE>
            <AMOUNT>1000.00</AMOUNT>
          </ALLLEDGERENTRIES.LIST>
        </VOUCHER>
      </TALLYMESSAGE>
    </DATA>
  </BODY>
</ENVELOPE>
```

**Expected:** Tally XML response with `<CREATED>1</CREATED>` indicating the voucher was created.

---

## 5. Common errors and what they mean

| Status | Body | What's wrong | Fix |
|---|---|---|---|
| 401 | `missing or invalid Authorization: Bearer <token>` | Token not sent, or wrong | Re-copy from tray menu |
| 415 | `Content-Type must be text/xml or application/xml` | Wrong Content-Type header | Add `-H "Content-Type: text/xml"` |
| 400 | `request body must not be empty` | No XML body in the POST | Add `--data-binary @file.xml` |
| 503 | `Tally is not reachable on port 9000` | Tally isn't running, or port 9000 isn't enabled | See section 2 |
| 502 | `HTTP call to Tally failed: ...` | Tally returned an error or timed out | Check Tally is responsive |

---

## 6. Test from a real browser

Use the test page: deploy `test-page/` or open it locally. It provides:
- Health check
- Pairing token entry
- XML template dropdown (list companies, list ledgers, list vouchers, create voucher)
- Raw XML textarea for custom queries
- Response display

Until the cert is trusted system-wide, the first browser hit will show a **"Your connection is not private"** warning. Accept it once at `https://127.0.0.1:9100/health`.

---

## 7. Uninstall

Windows **Settings -> Apps -> Installed apps -> Lekha AI Tally Connector -> Uninstall**.

This removes the binary, the auto-start entry, and the Start Menu shortcut. **It does NOT delete the per-user data folder** at `%LOCALAPPDATA%\LekhaAI\TallyConnector\` (cert, key, pairing token). Delete that manually if you want a truly clean wipe.
