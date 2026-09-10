# Privacy

Quota Float is designed to be local-first and minimal.

## What the app reads

- The desktop app reads the local Codex login file from `CODEX_HOME/auth.json` or the user's `.codex/auth.json`.
- It sends the existing Codex access token only to the ChatGPT quota endpoints needed to read Codex usage.
- The existing usage response also supplies the credit balance. The app keeps it in memory for display and transient-failure recovery; displaying Credits adds no endpoint or stored account data.
- It may read the account identifier from the login file or token payload only to set the request header expected by the quota service.

## GNOME Shell extension boundary

The Ubuntu `.deb` includes a local GNOME Shell extension because ordinary Wayland clients cannot position one application window relative to another.

While Quota Float is running, the extension listens for GNOME top-level window position, size, and state changes and keeps a 250 ms top-level window check as a discovery/recovery fallback. It reads application identity, focus/minimized/workspace state, fullscreen/maximized state, and frame geometry so it can identify the Quota Float and ChatGPT windows and reserve space for the compact orb when needed. Finding those windows necessarily examines this limited metadata for other top-level windows; it does not read any window contents, titles, keyboard input, prompts, chats, or files, and it makes no network requests.

For a 100 × 100 compact orb, the extension uses that metadata to keep a 24 px right margin in GNOME desktop coordinates and move up 136 logical px from the default 24 px bottom anchor when the ChatGPT host is neither fullscreen nor fully maximized. With an effective geometry scale of 1, this produces a 160 px bottom margin; other scale and coordinate modes convert the offset from the widget's own geometry so the 136 logical px move is preserved. Fullscreen or fully maximized hosts use the default 24 px desktop-coordinate bottom margin; the 320 × 320 full panel always uses the default 24 px margins. This local coordinate conversion does not add private data or network activity. The extension does not inspect the host's input controls or perform pixel recognition. If the host is too short for the reserved area, the orb is clamped to the host's top edge. Position and size events use the same limited metadata and are not stored.

The extension moves, raises, minimizes/restores, and changes the workspace of the Quota Float window only. It never moves, minimizes, raises, or changes the workspace of ChatGPT or another application.

In a GNOME session, the app makes a best-effort attempt to enable the installed extension at startup and disable it on a normal exit. A crash, forced termination, or system policy can prevent cleanup, so users can also control it explicitly:

```bash
gnome-extensions disable quota-float-anchor@quotafloat.app
gnome-extensions enable quota-float-anchor@quotafloat.app
```

Removing the Ubuntu package removes the extension files:

```bash
sudo apt remove quota-float-ubuntu
```

GNOME may require logging out and back in after an extension install, update, or removal.

## What it stores

Quota Float stores only widget preferences in its own application config directory:

- panel visibility and expanded/collapsed state;
- always-on-top preference;
- pinned provider and auto-rotate interval;
- selected quota window (5-hour or weekly);
- interface language.

Older preference files may contain the retired `locked` field. Current versions ignore it and omit it the next time preferences are saved.

If the user enables Start at login, the autostart plugin creates the platform's standard login-start registration, such as an XDG autostart entry on Linux. Disabling the option removes that registration.

It does not copy or persist Codex tokens, account IDs, raw quota responses, user prompts, chat history, or local auth paths.

## What it sends

The app only calls these quota-related HTTPS endpoints from the local desktop process:

- `https://chatgpt.com/backend-api/wham/usage`
- `https://chatgpt.com/backend-api/wham/rate-limit-reset-credits`

No telemetry, analytics, crash reporting, or third-party tracking is included.

## Logging and accuracy

Logs are intentionally generic and must not include tokens, account IDs, raw backend responses, request headers, local auth paths, or personal file paths. Quota Float displays quota windows returned by the Codex quota service; it does not estimate quota from local token usage or fabricate values when the response shape is unknown.
