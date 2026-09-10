# Quota Float 0.1.12-2

Quota Float is a cross-platform desktop widget for checking Codex quota from an existing local Codex login.

## Downloads

- Windows MSI + NSIS archive: `quota-float-windows-unsigned.zip`
- macOS universal app + DMG archive: `quota-float-macos-universal-unsigned.zip`
- Ubuntu 26.04 x86_64 Debian package: `Quota Float Ubuntu_0.1.12-2_amd64.deb`
- SHA-256 checksums: `SHA256SUMS.txt`

## What's new

- Follows the active ChatGPT host smoothly while it moves or resizes by coalescing position, size, and state events into the next GNOME redraw.
- Retains a 250 ms host discovery/recovery check as a fallback, while following only the current host and widget.
- Cleans up host/widget subscriptions and pending frame work when the host changes, the widget is destroyed, or the extension is disabled.
- Preserves the existing compact-orb avoidance rules for ordinary, fullscreen, maximized, tiled, and short hosts; the full panel remains anchored at the host's lower-right corner.
- Uses host frame geometry and window state without reading window contents or performing pixel recognition; fractional scaling and display movement remain real-machine checks.
- Retains the existing 0.1.9, 0.1.10, 0.1.11, and 0.1.12 Debian test packages unchanged when the 0.1.12-2 package is built.

## Ubuntu install

```bash
sudo apt install "./Quota Float Ubuntu_0.1.12-2_amd64.deb"
```

After a first extension install or update, log out and back in once. In a GNOME session, the app makes a best-effort attempt to enable the extension at startup and disable it on a normal exit; it can be disabled manually after a crash or forced termination.

## Release checks

- [ ] Frontend tests and web build passed.
- [ ] Rust formatting, Clippy, and tests passed.
- [ ] Windows MSI/NSIS and macOS universal app/DMG passed.
- [ ] Ubuntu `.deb` dependencies, extension files, unpacked privacy scan, and installation passed.
- [ ] GNOME multi-window anchoring, smooth movement/resize following, two-size constraint, tray recovery, refresh, and extension lifecycle passed.
- [ ] Compact placement was checked for ordinary, fullscreen, fully maximized, half-screen/tiled, single-axis-maximized, and very short hosts.
- [ ] Fractional scaling, cross-display movement, and display/workspace changes preserve the compact offset (interaction check pending).
- [ ] 5-hour/weekly tray switching, repeated selection, language switching, compact view, and saved choice after restart passed.
- [ ] Legacy `locked: true` preferences remain interactive and the Unlock widget tray item is absent.
- [ ] Windows/macOS desktop regression passed.
- [ ] Existing 0.1.9, 0.1.10, 0.1.11, and 0.1.12 Debian packages and checksums remain unchanged.
- [ ] Attachment set and `SHA256SUMS.txt` reviewed before publishing.
