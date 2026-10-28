# tempmon

Hardware temps, CPU cap, and live per-app FPS for GNOME (Wayland or X11), with an always-on-top overlay and a game-session logger.

- `src/` — the GTK3 app (`tempmon`, `tempmon --overlay`)
- `fps-probe/` — GNOME Shell 46 extension that counts frames per window and writes `$XDG_RUNTIME_DIR/fps-probe.json`
- `system/` — `cpu-cap` script, its boot service, and the sudoers rule that lets the app change the cap without a password

## Install

```sh
cargo build --release

sudo install -d /usr/share/gnome-shell/extensions/fps-probe@itzzsuperrr
sudo install -m 644 fps-probe/* /usr/share/gnome-shell/extensions/fps-probe@itzzsuperrr/
gnome-extensions enable fps-probe@itzzsuperrr   # takes effect after next login

sudo install -m 755 system/cpu-cap /usr/local/bin/
sudo install -m 644 system/cpu-cap.service /etc/systemd/system/
sudo install -m 440 system/cpu-cap.sudoers /etc/sudoers.d/cpu-cap
sudo systemctl enable cpu-cap.service
```

Game sessions are logged to `~/.local/share/tempmon/games.log`.
