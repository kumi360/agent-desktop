# Nimbus UI Pilot Test Plan (fdesk)

## Objective
Validate fdesk against the real Nimbus web UI in a controlled pilot before
any unsupervised testing.

## Prerequisites
- Nimbus UI running in a safe environment (staging, not prod)
- A test account with no real data
- Mac with Accessibility permission (local Terminal, not SSH)
- Jev API reachable

## Pilot Tasks (in order, easiest first)

### P1: Smoke — load and verify
```
python -m fdesk --app <Browser> --web --task "Navigate to the Nimbus staging URL and verify the login page loads" --text <url>
```
Success: login form visible, DONE within 10 steps.

### P2: Login
```
python -m fdesk --app <Browser> --web --task "Log in with the test account" --text <username> --text <password>
```
Success: lands on dashboard/home, DONE. Watch for: 2FA, captchas.

### P3: Core action
(Pick ONE core Nimbus workflow, e.g. "create a new X", "search for Y")
```
python -m fdesk --app <Browser> --web --authorize --task "<workflow>" --text ...
```
Success: workflow completes. Note any stalls, wrong clicks, or abstains.

## What to record per task
- Steps taken, time, Jev calls, cost
- Where it stalled or chose wrong (screenshots if possible)
- Confidence scores on key decisions (from logs)
- Any PERM_DENIED, STALE_REF, or GATE_REFUSED events

## Kill criteria (stop the pilot)
- Destructive action attempted without --authorize
- Loop exceeds 40 steps on a simple task
- Same wrong action 3x (repeat circuit breaker should catch this)
- Any data written to prod (should be impossible in staging)

## After pilot
Calibrate thresholds from observed confidence scores. Document Nimbus-specific
control patterns (dynamic IDs, iframes, etc.) for the next hardening round.
