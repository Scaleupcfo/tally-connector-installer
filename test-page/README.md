# Test page — Lekha Tally Agent

A standalone web page that mimics what Lekha AI will eventually do: download the agent, pair via token, call the agent's HTTPS endpoints, show responses. Hosted separately so we can hand the URL to Ashish / the CA without touching the main Lekha AI codebase yet.

## Run locally

```bash
cd test-page
npm install
npm start
# -> http://localhost:3000
```

(Port 3000 is already in the agent's CORS allowlist.)

## Deploy on Railway

1. Go to https://railway.app, sign in with GitHub.
2. **New Project → Deploy from GitHub repo** → pick `Scaleupcfo/tally-connector-installer`.
3. In the service settings:
   - **Root Directory:** `test-page`
   - **Build Command:** (leave blank — Railway autodetects npm)
   - **Start Command:** `npm start`
4. Click **Deploy**. Railway gives you a URL like `https://tally-connector-installer-production.up.railway.app`.
5. Open that URL in a browser — you should see the test page.

The agent's CORS already allows any `*.up.railway.app` host, so no agent rebuild is needed.

## After deploy

Send the Railway URL to whoever needs to test. They:
1. Open the URL in Chrome/Edge
2. Click "Download installer" → install
3. Accept the cert on `https://127.0.0.1:9100/health` once
4. Paste pairing token (from tray menu)
5. Click each endpoint button to test
