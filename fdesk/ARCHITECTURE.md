# fastdesktop — Jev-driven desktop agent: architecture

## 1. Goal
A fast, cheap, autonomous desktop agent. The agent loop sits ON TOP of the
`agent-desktop` CLI (unchanged driver layer). One Jev SystemOne call per step
picks operation+target from an indexed accessibility tree; a planner LLM is
used only for planning, text generation, and verification. Target: sub-second
median step, sub-cent per step, accuracy ≥ agent-desktop's contract.

Non-goals: modifying agent-desktop itself; screenshots/pixel grounding;
replacing the planner tier.

## 2. System layout

```
┌─────────────┐     ┌──────────────┐     ┌─────────────┐     ┌──────────────┐
│  Planner    │────▶│  Agent loop  │────▶│  Jev        │     │ agent-desktop│
│  (LLM:      │◀────│  (this repo) │◀────│  (SystemOne │     │ CLI (driver) │
│  Grok/Codex │     │              │     │  via OR)    │     │              │
└─────────────┘     └──────┬───────┘     └─────────────┘     └──────▲───────┘
                           │ exec snapshot/click/type               │
                           └────────────────────────────────────────┘
```

- **Driver**: `agent-desktop` CLI (npm). We shell out; its JSON contracts
  (`delivered_verified`, `STALE_REF`, `APP_NOT_FOUND`, …) are load-bearing.
- **Agent loop** (`fastdesktop/`): observe → index → decide → gate → act → verify.
- **Jev**: `typesafe/jev-router` (or `jev-1.13`) via OpenRouter SystemOne
  (`POST /api/v1/systemone`). Transport already proven via local proxy.
- **Planner**: any OpenAI-compat LLM (Grok/Codex); called rarely.

## 3. The loop (per step)

1. **Observe**: `agent-desktop snapshot --app <name> [--window-id <id>] --json`.
   Keep the raw JSON. Record `snapshot_id`, `complete` flag, focused window.
2. **Index**: project the AX tree into a control table:
   `{id: "@s1:e7", role, label, value?, states?, window, context (window/group title)}`.
   Cap: 320 controls max, 160 offered (mirror fastbrowse's ladder; recalibrate).
   If `complete:false` → DRILL (re-snapshot pinned to a region/window) or WIDEN.
3. **Decide**: one Jev `evaluate(state, questions)` call, speculative fan-out:
   - `operation`: choice over offered ops {CLICK, TYPE_TEXT, SET_VALUE, PRESS_KEY,
     FOCUS_WINDOW, SCROLL, OPEN_APP, WAIT, DONE, BLOCKED} (+DRILL/WIDEN).
   - `<op>_target`: choice over indexed control ids (criterion = id/label/role/
     context/value). Only the matching head is consumed.
   - `present`, `destructive`, `needs_text`: noul risk gates.
   - State: `{task, app, window, elements[], recent_actions[≤10], notes}`.
   - Strict validation: choice ∈ offered, probabilities coherent, argmax wins.
4. **Gate** (code, never the model):
   - Confidence bars: ordinary ≥0.70 act; destructive (p≥0.50) ≥0.90;
     <0.55 never acts → {confirm, abstain}.
   - `--authorize` lifts ordinary gates, never destructive without explicit
     per-action approval surface.
   - Text: caller/planner-supplied only. Jev selects spans, never generates.
   - Mutations are never retried (`uncertainAction` receipt: may have landed).
5. **Act**: freshness gate (re-check app pid/window title/target still present),
   then the agent-desktop command. Record the delivery receipt.
6. **Verify**: fresh snapshot → settle (two agreeing snapshots) → diff effect
   into history ("what the action visibly did"). 3 no-change turns → BLOCKED.
   Done-check: Jev `complete` noul per requirement; holistic ≥0.85 done.

## 4. Desktop safety taxonomy (gates)

Irreversible (always Jev-judged + approval surface): file delete/trash-empty,
settings/preferences changes, sending/posting from a native app, app quit with
unsaved work, anything touching credentials or payments.
Never gated: read-only snapshot, focus, scroll.
Secrets: planner supplies names only; values resolved at dispatch, scoped to
app/bundle-id (desktop analog of fastbrowse's origin scoping).

## 5. Error contracts (agent-desktop bar)

Every failure is a typed JSON error, never a silent wrong answer:
`UNKNOWN_REF`, `STALE_REF` (+retarget hint), `APP_NOT_FOUND`, `AMBIGUOUS_TARGET`
(+window ids), `GATE_REFUSED` (why), `BUDGET_EXCEEDED`, `BLOCKED` (reason).
No panics outside tests. A run that reports no findings is a failed run
(dogfood rule, inherited).

## 6. Budgets

Per step: 1 Jev call (~250ms target, ~$0.00004). Planner calls: ≤1 per 10 steps
typical. Run caps: 40 ops / 80 model calls / 3 no-change turns (inherit).

## 7. Build plan

- **Phase 1 (Python prototype)**: `fdesk/` — loop + Jev client + agent-desktop
  subprocess driver + gates. Validate on Linux Xvfb rig (GTK test app):
  Calculator 7+8, TextEdit type+readback, error rails. Measure: p50 step
  latency, $/task, task success rate vs me-driving baseline.
- **Phase 2 (harden)**: port to the fork (`kumi360/agent-desktop`) as a new
  crate/bin following the repo's CI/lint/test discipline; npm distribution
  alongside agent-desktop.
- **Phase 3 (Mac)**: grok-driven validation round on his M4 Max (Accessibility
  granted): Finder/Calculator/TextEdit + ego-lite probe.

## 8. Open questions for calibration (not blockers)

- Exact Jev confidence cutoffs on desktop tasks (start at lahfir's bars,
  recalibrate from recorded decision packets).
- DRILL/WIDEN trigger thresholds for `complete:false` trees.
- Planner model choice per task class (cost/quality).
