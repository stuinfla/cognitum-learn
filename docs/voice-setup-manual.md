# Voice setup — manual procedure (v0.5.7 / v0.5.8)

This is the CLI procedure for wiring up voice access on Cognitum Learn until the
`learn voice setup` browser wizard (v0.6.0) ships. About 30 minutes start-to-finish
if you've done it once; longer first time.

For an overview of which ecosystem to pick, see the [Voice setup](../README.md#voice-setup)
section in the main README.

---

## 1. Voice-proxy LaunchAgent (Mac)

The voice-proxy is a small HTTP service on your Mac that translates
*"ask cognitum X"* requests from Siri / Alexa / Google into `learn ask` calls
and returns the cited answer. It must bind to `0.0.0.0`, not `127.0.0.1`, so
ecosystem callbacks can reach it through the tunnel.

```bash
# Install the LaunchAgent (one time)
cp scaffolding/launchd/com.cognitum.voice-proxy.plist ~/Library/LaunchAgents/
launchctl load ~/Library/LaunchAgents/com.cognitum.voice-proxy.plist

# Verify it's bound to 0.0.0.0:7879
lsof -nP -i :7879
# Should show: cognitum-voice-proxy ... TCP *:7879 (LISTEN)
# If it shows 127.0.0.1:7879, set COG_VOICE_BIND=0.0.0.0 in the plist and reload.
```

The proxy reads its bearer token from `~/.cognitum-learn/voice-proxy.token`
(auto-generated on first install).

---

## 2. cloudflared tunnel

```bash
brew install cloudflared
cloudflared tunnel --url http://localhost:7879
# Note the printed *.trycloudflare.com URL — you'll embed it in the Shortcut / Skill below.
```

Quick tunnels (free, no Cloudflare account) get a new URL each invocation —
fine for Apple Shortcuts which can be re-installed, painful for Alexa which
binds the OAuth redirect to one URL per skill. For Alexa: register a free
Cloudflare account and create a *named* tunnel with a stable hostname.

---

## 3. Apple Shortcut (≤4 min, GA in v0.5.7)

1. On your iPhone, open the Shortcuts app → search "Cognitum" (after running
   `learn voice setup --ecosystem apple --emit-shortcut` once on your Mac,
   the install link is in your iCloud).
2. Open the Shortcut and edit the URL action to your cloudflared URL.
3. Set up a Siri phrase: long-press the Shortcut → Add to Siri → say
   *"ask Cognitum"* or *"hey Cognitum"*.
4. Test on HomePod / CarPlay: *"Hey Siri, ask Cognitum what temperature for
   medium-rare steak"*.

Works on iPhone, HomePod, CarPlay, and Apple Watch. Free. No developer account
required.

---

## 4. Alexa Custom Skill (≤5 min, in flight v0.5.8)

This path uses a private Custom Skill on your own Amazon developer account
(free under the AWS free tier, 1M requests/month).

```bash
# Install Amazon's ask-cli tool
npm install -g ask-cli
ask configure       # OAuth into your Amazon developer account

# Deploy the Cognitum skill scaffold
cd scaffolding/voice-proxy/alexa/
ask deploy
# Note the skill ID; you'll need it for testing.
```

The skill's Lambda function calls your cloudflared tunnel. The v0.5.8 release
adds a Haiku-fast-path that returns within Alexa's 8-second response window
for short queries; long queries fall back to *"let me think about that for
a moment."*

Test: *"Alexa, ask Cognitum about laminating dough"*.

---

## 5. Google Routines (≤6 min, scripted-only)

**Important:** arbitrary slot Q&A on Nest hardware is *not possible* — Google
retired Conversational Actions on June 13, 2023 with no replacement. What
*does* work is pre-defined Routines that broadcast a fixed TTS answer.

Three patterns ship as scaffolding:

1. **"Hey Google, run Cognitum check"** — generic Q&A surface
2. **"Is the room safe?"** — uses RuView presence sensors + KB context
3. **"Good morning Cognitum"** — daily briefing from your KB

Setup: open the Google Home app → Routines → New → trigger on the phrase →
action: send command to Home Assistant → Home Assistant runs a script that
hits your voice-proxy and broadcasts the TTS via `notify.google_assistant_sdk`.

For the TTS broadcast to work, you need a `homegraph` service account JSON
at `~/.homeassistant/google_service_account.json`. Generate it from the
Google Cloud Console (Home Graph API service account) and download the JSON.

---

## Dimension fix (referenced from troubleshooting)

If `learn push <topic>` fails with *"dimension mismatch: expected N, got 384"*,
your Seed's vector store was first written to at dimension N — not 384, which
is what BGE-small embeds at. The fix is to align the agent's `--dimension`
flag with 384 and wipe the old store.

```bash
# SSH to your Seed
ssh genesis@cognitum-XXXX.local

# Edit the dimension override
sudo sed -i 's|--dimension [0-9]*|--dimension 384|' \
  /etc/systemd/system/cognitum-agent.service.d/override-dimension.conf

# Apply: reload systemd, stop agent, wipe store, restart
sudo systemctl daemon-reload
sudo systemctl stop cognitum-agent
sudo rm -f /var/lib/cognitum/rvf-store/*.rvf
sudo systemctl start cognitum-agent

# Verify on Mac
learn doctor
learn push <topic>
```

This is *not* a firmware bug. The Seed's dimension is set by the agent's
`--dimension` flag on first write and locked thereafter. Never chase this as
an OTA / firmware issue — fix the flag.

---

## Troubleshooting voice paths

| Symptom | Likely cause | Fix |
|---|---|---|
| iPhone Shortcut returns nothing | Voice-proxy bound to 127.0.0.1 | `COG_VOICE_BIND=0.0.0.0`, reload LaunchAgent, verify with `lsof -nP -i :7879` |
| Alexa says *"Cognitum is having trouble"* | Lambda cold-start exceeded 8s | Enable Haiku-fast-path in v0.5.8; watch `aws logs tail /aws/lambda/ask-cognitum --follow` |
| Google Routine fires, speaker silent | Missing `homegraph` service account JSON | Place JSON at `~/.homeassistant/google_service_account.json` |
| cloudflared URL changed | Quick tunnel restarted | Re-install the Shortcut, or migrate to a named tunnel |

---

When v0.6.0 ships, `learn voice setup` will replace all of the above with a
single browser wizard. Until then, this is the canonical manual path.
