# fdesk — Jev-driven desktop agent (prototype)

Python prototype of the autonomous desktop-agent loop. Drives the
`agent-desktop` CLI; one Jev SystemOne call per step picks the operation
and target from the accessibility tree.

## Layout
- `fdesk/driver.py` — subprocess wrapper around `agent-desktop`
- `fdesk/jev.py` — SystemOne client (choice/noul/score)
- `fdesk/policy.py` — control indexing, question building, routing, fingerprints
- `fdesk/gates.py` — destructive-action taxonomy, caller-supplied text
- `fdesk/loop.py` — observe → decide → gate → act → verify
- `fdesk/cli.py` — `python -m fdesk`

## Run
```bash
export JEV_BASE_URL=https://openrouter.ai/api/v1
export JEV_API_KEY=<key>          # OpenRouter key
export JEV_MODEL=jev-1.13
python -m fdesk --app TextEdit --task "Type hello into the document" --text hello
```

## Status
Prototype validated on a Linux Xvfb rig (see `TEST-PLAN.md`). Not production
code — the hardening pass (error contracts, CI, safety gates) is next.
