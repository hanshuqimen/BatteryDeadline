# Releasing

1. Update matching versions in `package.json`, workspace `Cargo.toml`, and `src-tauri/tauri.conf.json`; update the changelog.
2. Run all README checks. Build the Windows desktop and CLI using locked dependencies, then run `scripts/package-release.ps1`.
3. Review simulator labels, the validation record, pending recovery behavior, package contents and checksums. Launch the packaged executable in simulation and real read-only mode. Hardware acceptance is separate.
4. Commit, push, and tag `vX.Y.Z`. The tag release workflow reuses Windows/portable-core CI, downloads its artifacts and creates a GitHub prerelease with the installer, portable ZIP and checksums. Set the preview status intentionally for a future stable release.

Artifacts are generated from source in GitHub Actions; binaries and private data are not committed. Current initial builds are unsigned. Do not claim signing, guaranteed runtime or cross-device validation without evidence. A Microsoft WebView2 bootstrapper may need network access during installation; normal application operation is local.

The Actions workflow uses `contents: read` for checks and `contents: write` only for publishing the release. Third-party action references are pinned to commit SHAs. A maintainer may manually upload locally verified artifacts to the same release; automation replaces like-named assets from its own verified build.
