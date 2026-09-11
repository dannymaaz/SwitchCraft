# SwitchCraft

**Switch between your Codex, Gemini, and Claude accounts in one click.**

Tired of logging out and back in every time you need a different account? SwitchCraft sits in your system tray and lets you swap credentials instantly — no browser needed, no terminal gymnastics.

![SwitchCraft Screenshot](https://raw.githubusercontent.com/dannymaaz/SwitchCraft/main/docs/screenshot.png)

---

## Why SwitchCraft?

If you're juggling work and personal accounts across Codex, Gemini (Antigravity), or Claude — whether CLI or desktop app — you know the pain. SwitchCraft keeps all your profiles in one place and switches with a single click.

- **Codex** (CLI & Desktop) — swaps `~/.codex/auth.json`
- **Gemini / Antigravity** (CLI & IDE) — swaps `~/.gemini/tokens.json`
- **Claude** (Desktop & Code) — swaps session credentials

No more `rm ~/.codex/auth.json && codex login`. Just click.

---

## Features

- **One-click account switching** — select a profile, done
- **Multi-platform** — manage Codex, Gemini, and Claude profiles side by side
- **Usage tracking** — see your remaining weekly/hourly limits at a glance
- **Import current session** — grab your active credentials automatically
- **System tray** — runs silently in the background, always ready
- **Auto-updates** — get notified when a new version drops, update without losing your data
- **Launch at startup** — optional, starts hidden in the tray
- **Cross-platform** — Windows, macOS, and Linux
- **Tiny footprint** — under 10 MB, uses ~20 MB RAM

---

## Install

### Download

Grab the latest release for your OS:

| Platform | Download |
|---|---|
| Windows | [`.msi` installer](https://github.com/dannymaaz/SwitchCraft/releases/latest) |
| macOS | [`.dmg` disk image](https://github.com/dannymaaz/SwitchCraft/releases/latest) |
| Linux | [`.AppImage` / `.deb`](https://github.com/dannymaaz/SwitchCraft/releases/latest) |

### Install from terminal

**macOS / Linux:**

```bash
curl -fsSL https://raw.githubusercontent.com/dannymaaz/SwitchCraft/main/scripts/install.sh | bash
```

**Windows (PowerShell):**

```powershell
irm https://raw.githubusercontent.com/dannymaaz/SwitchCraft/main/scripts/install.ps1 | iex
```

---

## Quick start

1. Open SwitchCraft (it appears in your system tray)
2. Click **Add** to create a profile
3. Pick your platform (Codex, Gemini, or Claude)
4. Either paste your credentials JSON or click **Import current session** to grab whatever's active
5. Add as many profiles as you need
6. Click any profile to switch — that's it

Your original credentials are backed up automatically before every switch.

---

## How it works

SwitchCraft doesn't run a proxy or intercept anything. It's straightforward file management:

1. When you switch to a profile, SwitchCraft backs up your current auth file
2. It writes the selected profile's credentials to the expected path
3. The next time you open your CLI or desktop app, it picks up the new credentials

All profiles are stored locally in `~/.switchcraft/accounts.json`. Nothing leaves your machine.

---

## Settings

- **Launch at startup** — starts SwitchCraft minimized to tray when your computer boots
- **Auto-check for updates** — pings GitHub Releases on launch to see if there's a new version
- **Switch notifications** — shows a confirmation toast when you switch accounts

---

## Building from source

You'll need [Rust](https://rustup.rs/) and [Node.js](https://nodejs.org/) (v18+).

```bash
git clone https://github.com/dannymaaz/SwitchCraft.git
cd SwitchCraft
npm install
npm run tauri dev
```

To build a release:

```bash
npm run tauri build
```

---

## Updating

SwitchCraft checks for updates automatically (you can turn this off in Settings). When a new version is available, a banner appears at the top of the app. Click "Update now" — it downloads, installs, and restarts without losing your profiles.

You can also check manually from the tray menu or Settings page.

---

## Contributing

Found a bug or want to add support for another tool? PRs are welcome.

1. Fork the repo
2. Create a branch (`git checkout -b fix/something`)
3. Make your changes
4. Open a PR

Please keep commits clean and test on at least one platform before submitting.

---

## License

MIT — see [LICENSE](LICENSE) for details.

---

**Powered by Danny Maaz**
