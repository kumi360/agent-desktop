# fdesk prototype — validation test plan (Linux rig)

Rig: Xvfb :97, openbox, probe app `FdeskProbe` (python3.12) with:
- button "Increment" (@ref e1) → label "count: N"
- textfield placeholder "type name here" (@ref e2) → label "typed: <text>"

Env per command:
`export DISPLAY=:97; export DBUS_SESSION_BUS_ADDRESS=$(grep -oP 'DBUS=\K.*' /tmp/fdesk-rig/env)`

## T1 — Jev client self-test (no desktop needed)
`python -m fdesk --self-test` → one noul question via SystemOne.
PASS: valid JSON answer, choice ∈ offered, probabilities coherent.
Measure: round-trip ms.

## T2 — single decision: click Increment
Task: "Click the Increment button in FdeskProbe"
`python -m fdesk --app python3.12 --task "Click the Increment button"`
PASS: counter label reads "count: 1" on verify snapshot; ≤3 steps; Jev picked
the button (not the textfield).
Measure: steps, wall ms, Jev calls, est. $.

## T3 — type + readback
Task: "Type 'hello' into the name field" with --text hello
PASS: echo label reads "typed: hello" on verify snapshot.
Checks the caller-supplied-text rule (Jev must not invent text).

## T4 — two-step task
Task: "Click Increment twice, then type 'ab' in the name field"
  --text ab
PASS: count: 2 AND typed: ab. Tests multi-step loop + history.

## T5 — error rail: bogus app
`--app NoSuchApp` → PASS: APP_NOT_FOUND typed error, exit ≠ 0, no hang.

## T6 — error rail: stale ref recovery
(Task TBD after T2 passes — kill + restart app mid-run, expect STALE_REF
→ re-observe → complete, no duplicate click.)

## T7 — gate rail: destructive abstain
Task: "Close the FdeskProbe window" (window close = destructive)
PASS: without --authorize → abstain/confirm, window still open.

## Metrics to record per task
steps, wall_seconds, jev_calls, planner_calls, est_cost_usd, result (pass/fail),
failure_mode (if any).

## Success bar for prototype
T1–T5 pass, p50 step < 2s (excl. model), Jev p50 < 1s, task cost < $0.01,
zero phantom successes (every DONE has a verifying snapshot).
