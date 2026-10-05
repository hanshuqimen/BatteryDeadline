# Localization

The installed application contains Simplified Chinese (`zh-CN`), English (`en`), and Japanese (`ja`). It needs no network or translation service. `language: system` is the persisted default. The Rust backend reads the Windows user locale and sends the resolved locale in every snapshot, so the WebView and native tray use the same preference.

Settings exposes a language selector independent of the comfort-limit form. It saves immediately and is available during a session. Rust permits presentation-only preference changes while rejecting changes to a running session's reserve/control contract. Old settings deserialize with the system-language default.

`src/locales/*.json` are complete catalogs keyed by the English source message. TypeScript verifies all locales have every key; tests also verify nonempty values and matching interpolation parameters. User text is rendered as React text, not HTML. `src/i18n.ts` provides context, `Intl` formatters, parameter substitution and translation of stored controller activity. Original activity/diagnostic records stay language-independent; low-level Windows error codes and unknown technical details are preserved.

The native Rust tray uses the same locale resolver with compiled translations. Tray labels refresh on its regular update interval (up to 10 seconds). The per-user installer offers English, Simplified Chinese and Japanese resources; app language selection is independent of the installer's language.

To add another language, supply a complete catalog, extend the validated preference/resolver/TypeScript union, add native tray strings and installer resources, then run catalog, persistence and narrow-window tests. Keep the English and Chinese README links reciprocal. CLI machine-readable diagnostics and raw logs remain stable, developer-oriented output.
