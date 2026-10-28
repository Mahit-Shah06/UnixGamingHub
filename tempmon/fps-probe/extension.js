// Counts the frames every window hands to the compositor and writes them once a second to
// $XDG_RUNTIME_DIR/fps-probe.json for tempmon. Works for any app, already running or not,
// because every frame on screen passes through mutter.
import GLib from 'gi://GLib';
import Gio from 'gi://Gio';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

const OUT = GLib.build_filenamev([GLib.get_user_runtime_dir(), 'fps-probe.json']);
// one commit can arrive as several damage rects back to back; count each burst once
const COALESCE_US = 1000;

export default class FpsProbe extends Extension {
    enable() {
        this._actors = new Map();
        this._lastFlush = GLib.get_monotonic_time();
        this._addedId = global.window_group.connect('child-added', (_g, actor) => this._track(actor));
        for (const actor of global.get_window_actors())
            this._track(actor);
        this._timer = GLib.timeout_add_seconds(GLib.PRIORITY_LOW, 1, () => {
            this._flush();
            return GLib.SOURCE_CONTINUE;
        });
    }

    disable() {
        GLib.source_remove(this._timer);
        global.window_group.disconnect(this._addedId);
        for (const [actor, s] of this._actors) {
            actor.disconnect(s.damagedId);
            actor.disconnect(s.destroyId);
        }
        this._actors = null;
        try {
            Gio.File.new_for_path(OUT).delete(null);
        } catch {}
    }

    _track(actor) {
        if (!(actor instanceof Meta.WindowActor) || this._actors.has(actor))
            return;
        const s = {frames: 0, last: 0};
        s.damagedId = actor.connect('damaged', () => {
            const now = GLib.get_monotonic_time();
            if (now - s.last >= COALESCE_US)
                s.frames++;
            s.last = now;
        });
        s.destroyId = actor.connect('destroy', () => this._actors.delete(actor));
        this._actors.set(actor, s);
    }

    _flush() {
        const now = GLib.get_monotonic_time();
        const secs = Math.max((now - this._lastFlush) / 1e6, 0.1);
        this._lastFlush = now;
        const focus = global.display.focus_window;
        const tracker = Shell.WindowTracker.get_default();
        const windows = [];
        for (const [actor, s] of this._actors) {
            const w = actor.meta_window;
            if (w && (s.frames || w === focus)) {
                windows.push({
                    app: tracker.get_window_app(w)?.get_name() ?? '',
                    class: w.get_wm_class() ?? '',
                    title: w.get_title() ?? '',
                    pid: w.get_pid(),
                    fps: Math.round(s.frames / secs * 10) / 10,
                    focused: w === focus,
                });
            }
            s.frames = 0;
        }
        const json = JSON.stringify({ts: GLib.get_real_time() / 1000, windows});
        try {
            GLib.file_set_contents(OUT, new TextEncoder().encode(json));
        } catch {}
    }
}
