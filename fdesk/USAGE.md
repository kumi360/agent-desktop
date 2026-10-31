# fdesk — usage guide

## Install
```bash
npm install -g agent-desktop   # the driver (required)
export JEV_BASE_URL=https://openrouter.ai/api/v1
export JEV_API_KEY=<your-openrouter-key>
export JEV_MODEL=jev-1.13
```

## Run a task
```bash
python -m fdesk --app TextEdit --task "Type hello into the document" --text hello
python -m fdesk --app Finder --task "Click the Go menu"
```

## How it works
1. **Observe** — snapshot the app's accessibility tree via agent-desktop.
2. **Decide** — one Jev call picks the operation + target from indexed controls.
3. **Gate** — confidence bars (0.70 ordinary / 0.90 destructive / 0.55 floor).
   `--authorize` lifts ordinary gates only. Destructive actions (delete, send,
   settings, quit-unsaved, credentials) always need explicit approval.
4. **Act** — freshness check, then the agent-desktop command. Mutations are
   never retried (a repeat could double-click).
5. **Verify** — fresh snapshot, tree fingerprint diff. Three no-change turns,
   three identical actions, or 40 steps → stop.

Completion is a dedicated Jev choice question asked after each successful
action: "did this action fulfill what was asked?"

## Text rule
The agent never invents typed text. Text comes only from `--text` flags
(consumed in order) or quoted spans in the task string.

## Exit codes
- `0` — task completed (status: done)
- `1` — blocked, error, budget exceeded, or confirmation needed. The JSON on
  stdout carries a typed `error.code` (APP_NOT_FOUND, STALE_REF,
  GATE_REFUSED, REPEAT_ACTION, BUDGET_EXCEEDED, ...).

## Cost
~$0.0001 per step (one Jev call). A 3-step task costs about three-hundredths
of a cent and finishes in under 10 seconds.

## Tests
```bash
python -m fdesk.tests   # 43 hermetic unit tests, no display needed
```
