## Install

Grab the installer or the portable zip from
[releases](https://github.com/nyedle/ResolutionSwitcher/releases/latest). You
need one or the other.

The installer needs no admin rights and saves settings to
`%APPDATA%\ResolutionSwitcher\config.json`. The portable version keeps them in
its own folder, so you can copy that folder to another PC and take everything
with you.

Windows may warn you the first time. The app is not signed with a paid
certificate, and I am not paying for one.

## Using it

Pick a monitor, choose your settings, hit **Apply**. **Save as slot** keeps them
for later.

Click a slot's hotkey button and hold Ctrl or Alt with any key, or an F-key on
its own. Each hotkey belongs to one slot only.

Under **When a program opens**, name a game and pick a slot for when it starts
and another for when it quits.

## If it goes wrong

Every change asks you to keep it and undoes itself after 15 seconds if you do
not. If the screen goes black, just wait.

**Reset** puts one monitor back to normal. **Ctrl + Alt + Shift + R** does every
monitor at once, from anywhere, even with the app in the tray.

## Updates

It checks GitHub on startup and installs anything newer in the background. The
new version starts next time you open it. Turn that off in Settings and it never
touches the internet.

## Building

You need [Rust](https://rustup.rs) and Windows.

```sh
cargo test
cargo run
```

Run `cargo fmt` and `cargo clippy --all-targets -- -D warnings` before a pull
request, since CI fails on them.

For an installer and portable zip, with
[Inno Setup](https://jrsoftware.org/isdl.php) on your PATH:

```powershell
powershell -File scripts\pack.ps1 -Arch x64 -Version 1.0.0
```

Most of the display code cannot be tested automatically because it talks to real
hardware. If you touch it, try it on a real monitor and say so in the pull
request.

To release, bump the version in `Cargo.toml`, add a `## <version>` section to
[CHANGELOG.md](CHANGELOG.md), then push a tag. CI refuses the tag if those three
disagree, then builds both architectures and publishes everything. The release
notes come from the changelog section.
