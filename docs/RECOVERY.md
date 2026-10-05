# Recovery and manual changes

Original settings and intended values are saved to SQLite before Windows is modified. Successful writes must be read back. Restoration walks the journal in reverse, checks device identity and compares the current value against the last value the app wrote.

| Trigger | Behavior |
| --- | --- |
| Stop / tray Quit / normal exit | Restore before completing the operation |
| AC connected or power source unknown | Restore and pause; auto-resume is opt-in |
| Deadline or reserve reached | End control and restore |
| Sampling gap over 15 seconds | Restore, reset estimates and warm up again |
| Force kill / crash | Recover on the next launch in the same mode |
| Original display unavailable | Keep pending entries; reconnect and select Restore now |
| User changes a managed value | Preserve the user's value and stop fighting it |
| Restore fails or is unverified | Keep pending entries and block new sessions |

Data lives under `%LOCALAPPDATA%\BatteryDeadline`:

- Real: `battery-deadline.sqlite3` plus its WAL/SHM and exclusive lock.
- Simulator: `simulation.sqlite3` and a separate lock.
- Logs: `logs` (daily rotation, up to seven files).
- Diagnostics: locally exported ZIPs in the diagnostics directory.

Restart the app, use Restore now, and check the Activity list. Reconnect the same display if it was disconnected. Do not remove the database, journal or WAL while recovery is pending. Export diagnostics locally if recovery remains blocked.

The app duplicates the active power scheme for DC CPU changes. On restoration it reactivates the original and deletes its inactive private clones. If you select another scheme, your selection wins. If you deliberately edit the active private clone, it is treated as your adopted plan and may remain; inspect it in Windows power settings rather than expecting the application to delete your change.

The browser's Rehearse recovery button performs a labeled stop/restore rehearsal. Automated restart tests actually drop and reopen an engine against persistent storage and a shared fake hardware state; they also cover a crash after the journal commit and before the applied marker. No button falsely claims to have killed a process.
