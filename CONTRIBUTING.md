# Contributing

Start with the simulator. Do not test unreviewed actuator changes on a real laptop. Read [Architecture](docs/ARCHITECTURE.md) and [Recovery](docs/RECOVERY.md) before changing hardware behavior.

For bug reports, include Windows build, laptop/panel model, power source, app version, steps, and observed versus expected behavior. Diagnostic archives may contain device identifiers and local activity: inspect and redact before sharing. Never attach the SQLite recovery journal publicly.

Keep estimation and controller decisions in the Rust core. Every actuator must detect support, snapshot the original, durably prepare a journal record, validate immediately before writing, verify its result, and handle restoration/manual overrides. Add behavioral regression coverage when changing these contracts; a mocked success-only test is insufficient.

Run the checks in [README](README.md) before submitting a pull request. Explain the user-visible change, validation, and remaining hardware limitations. Cross-device measurements should state methodology and uncertainty; simulated savings are never production claims.
