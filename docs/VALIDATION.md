# Validation record

Initial local validation: 2026-10-05, Windows x64. This separates verified software behavior from outstanding hardware field tests.

## Completed

- Rust workspace format check, strict Clippy (`-D warnings`), and 21 passing core unit/integration tests.
- Tests cover reserve math, spike filtering, capacity/percentage fallbacks, hysteresis, actual engine restart, pending-before-applied recovery, failed writes, manual overrides, unavailable displays, new-session blocking, exclusive storage, AC, deadlines and sleep-like gaps.
- Seven frontend behavioral tests cover midnight date handling, deadline validation, preset boundaries, chart gaps, complete translation catalogs, parameters, and locale selection. Strict TypeScript, ESLint and Prettier passed.
- The 600-second CLI simulation converges from 13 W toward its deadline budget, then plugging in restores all supported settings with no pending recovery.
- Read-only physical-laptop probe: AC connected, battery present, absolute remaining/full/design energy and voltage available. No discharge watts were reported on AC, as expected. Internal brightness, legal refresh modes, and active DC CPU policy were detected. Device identifiers and raw measurements are not published.
- Windows desktop executable and per-user NSIS installer built successfully. The installer is approximately 3.3 MB, excluding a missing WebView2 runtime download.
- Production dependency audit: `pnpm audit --prod` reported no known vulnerabilities at validation time.
- `cargo audit` reported no vulnerability-class findings. Its informational warnings concern `glib 0.18` unsoundness and unmaintained `proc-macro-error` in Tauri's Linux dependency graph; neither is compiled into this Windows target. Linux desktop distribution is not supported.
- The packaged desktop launched successfully against the real read-only Windows backend on AC, with the four-page UI and truthful unavailable discharge values.
- Browser walkthrough verified English, Chinese and Japanese selection, an active session continuing after a language change, saved language/theme after reload, translated history, and a local diagnostic ZIP containing only capabilities, version and application events.
- The simulator exercised bounded brightness/refresh/CPU adjustments, then AC restored settings. The recovery rehearsal completed and unlocked preferences. No browser warnings/errors were observed during this walkthrough.
- Chinese dark-mode preferences were inspected at a 620-pixel window: controls remained readable, with no horizontal overflow. README screenshots use clearly labeled synthetic telemetry.

This file does not treat a browser simulator as physical hardware validation.

## Not yet established

- Real discharge actuator savings and restore behavior across laptop vendors, hybrid graphics, multiple physical batteries, docking, Modern Standby, HDR, group policy and non-admin environments.
- Long-duration CPU <0.5%, memory, battery overhead and calibrated prediction accuracy. Architecture limits polling/storage work but is not a measured performance result.
- A reliable counterfactual for “energy saved.” Saved Wh remains unavailable instead of making a causal claim.
- Code signing and an independent security review. Initial artifacts are unsigned.

## Hardware acceptance checklist

Record original panel brightness, refresh, active plan and DC CPU maximum; unplug; run stable workload sessions for several hours; introduce a workload change; verify bounded actions, floors, prediction error and Stop/AC/deadline recovery. Repeat with sleep/resume, docking/display reconnection, manual edits, crash/restart and write-denied policies. Confirm no private plan accumulates after successful normal recovery. Capture methodology and device identifiers privately, then redact before sharing.

Do not describe this first preview as cross-device production-proven until those results exist.
