# Quota Float

[English](README.md) | [简体中文](README.zh-CN.md)

A small cross-platform desktop widget that reads the existing local Codex login state and displays remaining quota, reset time, and reset-credit information.

![Quota Float quota states](docs/images/quota-states.png)

## Platform support

- **Windows:** preserves the native ChatGPT/Codex host-following implementation and produces MSI and NSIS installers.
- **macOS:** produces universal Apple Silicon/Intel app and DMG bundles.
- **Ubuntu 26.04 x86_64:** produces a native `.deb` and uses a bundled GNOME Shell 48–50 extension to anchor the widget to the active ChatGPT window under Wayland.

Ubuntu support is an additional platform path. Shared React, quota, preference, tray, and release logic remains available to Windows and macOS.

## What it shows

- Codex plan, 5-hour or weekly remaining quota, the selected window's next reset time, and reset-credit information when the service provides it.
- Both full-panel quota views show **Credits** (**点数余额** in Chinese) between the reset-date line and reset-credit information. The amount uses the same `US$48.44` format in both languages and updates with quota refreshes. The USD display converts the service credit balance at 25 credits per US dollar; it is not the discounted purchase cost. Missing balances display `—`. See [conversion limitations](docs/KNOWN-LIMITATIONS.md).
- Healthy, caution, critical, stale, signed-out, and unavailable states.
- A 320 × 320 full panel and a 100 × 100 compact quota orb.
- Persistent preferences for panel visibility, expanded/compact mode, quota window, and interface language.
- The expanded panel's **5h**/**1w** badge toggles between the 5-hour and weekly quota pages without changing its appearance. The top-bar tray menu offers **5-hour quota** and **Weekly quota** with a check mark for the current choice. The full panel and compact orb switch together, and the choice survives restarts. Existing settings default to weekly quota.
- Tray controls also include show/hide, refresh, language, start at login, and quit.
- Manual refresh from error states and the tray, so recovery is not blocked by automatic retry backoff.

## Ubuntu installation

Download the Ubuntu 26.04 `.deb`, then install it with `apt` so runtime dependencies are resolved:

```bash
sudo apt install "./Quota Float Ubuntu_0.1.12-2_amd64.deb"
```

The supported Linux desktop session is Ubuntu 26.04 with GNOME Shell 48–50 on Wayland. Sign in to Codex on the same machine first. After the initial extension install or update, log out and back in once so GNOME Shell can scan it. In a supported GNOME session, Quota Float makes a best-effort attempt to enable the extension at startup and disable it on a normal exit. If system policy, a crash, or forced termination prevents cleanup, control it manually:

```bash
gnome-extensions disable quota-float-anchor@quotafloat.app
gnome-extensions enable quota-float-anchor@quotafloat.app
```

Uninstalling the package removes the app and extension files:

```bash
sudo apt remove quota-float-ubuntu
```

The Debian package depends on `libwebkit2gtk-4.1-0`, `libgtk-3-0t64`, `libayatana-appindicator3-1`, and `gnome-shell (>= 48)`; the bundled extension metadata currently supports GNOME Shell 48–50.

## GNOME Wayland placement

Wayland prevents an ordinary app from forcing an arbitrary cross-application coordinate. Quota Float therefore delegates placement to a small local GNOME Shell extension. The extension selects the focused ChatGPT window and preserves that host while the user interacts with the widget. The 320 × 320 full panel stays 24 px from the host's lower-right corner in GNOME desktop coordinates. For the 100 × 100 compact orb, a host that is neither fullscreen nor fully maximized keeps the 24 px right desktop-coordinate margin and moves the orb upward by 136 logical px from the default 24 px bottom anchor. On a conventional logical desktop with an effective geometry scale of 1, that is a 160 px bottom margin; other scale and coordinate modes convert the offset using the widget's own effective geometry scale so the upward logical distance remains 136 px. Fullscreen or fully maximized hosts restore the compact orb to the default 24 px desktop-coordinate bottom margin; half-screen/tiled and single-axis-maximized hosts keep the compact avoidance. A very short host clamps the orb to the host's top edge.

Placement and updates use the host window's frame geometry and fullscreen/maximized state. While the host moves or resizes, the extension listens for position, size, and state changes and coalesces updates into the next GNOME redraw. A 250 ms discovery/recovery check remains as a fallback; only the current host and widget are followed, and subscriptions plus pending frame work are cleaned up when either changes or the extension is disabled. It does not read window contents or perform pixel recognition.

The Linux window remains technically resizable so GTK accepts programmatic mode changes, but its native minimum and maximum are pinned to the selected 320 × 320 or 100 × 100 mode. Intermediate manual sizes are not allowed.

To identify the two windows, the extension examines limited identity, focus, workspace, minimized-state, and frame metadata from GNOME's top-level window list. It does not read window contents or use network access. See [PRIVACY.md](PRIVACY.md) for the exact boundary and lifecycle.

## How quota access works

Quota Float reads one existing local login file:

- `$CODEX_HOME/auth.json`, when `CODEX_HOME` is set;
- otherwise `~/.codex/auth.json`.

It uses that existing session only to call the Codex/ChatGPT quota endpoints. It does not estimate quota from token counts, redeem reset credits, change account settings, or save credentials. Browser preview uses mock data; real quota reads require the Tauri desktop app and a local Codex login.

The 5-hour and weekly allowances are matched by their window durations. If the service omits the selected window, the widget reports that window as unavailable instead of substituting the other percentage.

## Development

Install Node.js 20.19+ or 22.12+, Rust stable, and project dependencies, then run:

```bash
npm ci
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --locked --manifest-path src-tauri/Cargo.toml
npm run tauri -- dev
```

Ubuntu 26.04 additionally needs the Tauri GTK/WebKit development packages listed in [the release guide](docs/RELEASE.md).

## Build

Build the native bundles for the current Windows or macOS host with:

```bash
npm run tauri -- build
```

Build the Ubuntu Debian package with:

```bash
export RUSTFLAGS="--remap-path-prefix=$PWD=/src --remap-path-prefix=$HOME=/build"
npm run tauri:ubuntu
```

The command builds with the committed Cargo lockfile, normalizes the runtime dependency field, then extracts and verifies the package contents. The resulting `Quota Float Ubuntu_<version>_amd64.deb` is written under `src-tauri/target/release/bundle/deb/`, alongside the existing 0.1.9, 0.1.10, 0.1.11, and 0.1.12 Debian test packages, which are retained unchanged. The path remapping allows the package verifier to confirm that local build paths were not embedded. Linux intentionally targets `.deb` instead of AppImage; that Linux-specific choice does not remove Windows or macOS bundles.

## Release

CI covers the shared frontend plus Windows, macOS, and Ubuntu desktop builds. A matching `v*` tag creates a draft release containing:

- a Windows archive with one MSI and one NSIS installer;
- a macOS archive with one universal app and one DMG;
- one `Quota Float Ubuntu_<version>_amd64.deb`;
- `SHA256SUMS.txt`.

The Ubuntu gate extracts the `.deb` and scans the installed file tree for forbidden files, high-confidence secrets, and local build paths before upload. See [docs/RELEASE.md](docs/RELEASE.md) before publishing.

## License

MIT
