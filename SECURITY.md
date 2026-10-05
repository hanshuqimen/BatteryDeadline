# Security

The initial 0.1.x release is supported on Windows 11 x64. The application has no remote service, updater, arbitrary path IPC, shell plugin, or elevation request. Desktop commands accept typed arguments and the Rust backend validates settings and deadlines independently of the UI. Hardware calls run on one worker thread.

Frontend assets are local. The production CSP restricts scripts to packaged assets and connections to Tauri IPC. Inline styles are allowed for React chart/timeline dimensions; inline scripts are not. The development loopback HTTP bridge always uses synthetic hardware and is not packaged in the desktop.

Keep SQLite WAL/FULL durability and exclusive mode locks when modifying recovery. Never erase pending records to make an error disappear. A failed or unverified restoration blocks new sessions. User manual changes take precedence over obsolete originals.

Report a vulnerability through GitHub's private vulnerability reporting on this repository if available. Otherwise open an issue requesting a private contact without publishing exploit details, battery device identifiers, or recovery contents. This repository does not invent a contact address or claim an independent security audit.

Dependencies are locked by `Cargo.lock` and `pnpm-lock.yaml`; CI actions are pinned to commits. Run `pnpm audit --prod` and review Rust advisories when updating dependencies. Unsigned binaries are disclosed in release notes.
