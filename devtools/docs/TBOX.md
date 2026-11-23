# T box: type a note from inside the game

Press **T** while the game window is in front. A small box opens at the bottom of the game window, focused, showing
the last reply line from `notes\messages.jsonl`.

- Enter sends, Shift+Enter adds a line, Esc cancels. Focus goes back to the game either way.
- A green "note saved" toast shows for about a second and a half.
- Nothing is sent for an empty box.

## Start

`python devtools/start.py` starts it with the Mod UI (it is the default).

- `--no-tbox` leaves it out; `--tbox-key F9` uses another key.
- On its own: `python -I ootrr/tbox.py <session> [--key T] [--proc longshot.exe]`. It exits with the panel.
- It warns at start when any `longshot_options.ini` under the session binds the key (T is unbound in the default keymap,
  `key_default` in the Longshot source).
- Windows only (ctypes plus tkinter, standard library; no packages).

## What it writes

One line appended to `<session>\notes\notes.jsonl`, the same file and shape as the panel's own notes:

    {"kind": "note", "step": null, "text": "...", "verdict": null, "source": "t-box", "map": "...", "pos": [x, y, z],
     "life": 80, "event": false, "shot": null, "time": "14:58:19", "t": 1763800699.1}

`map`, `pos`, `life` and `event` come from `notes\state.json`, which the panel rewrites each time its page polls. The T box
only reads that file (no bridge call, nothing sent to the game) and leaves the fields out when the file is older than 15 s.
`source` is `t-box` so these can be told from the panel's note button. There is no screenshot.

## How a reader (a person or a program) uses it

Anything can follow the file; each new `"source": "t-box"` line is one typed note:

    tail -n 0 -F "<session>/notes/notes.jsonl" | grep --line-buffered '"source": "t-box"'

Replies go to `<session>\notes\messages.jsonl` (`{"text", "time"}`), which the panel page and the box both show.

## Does typing move Link? No.

Checked in Longshot's `host\nn_hid.c`. The keyboard is read with `GetAsyncKeyState`, which sees keys whatever window has
focus, but the read is gated by `game_in_front()`: when the foreground window belongs to another process the keyboard
and mouse are skipped (only pads bound to work in the background, and scripted or bridge input, still count). The box is
its own process, so while it has focus the game ignores the keys. Consequences:

- The box must stay a plain window. Longshot treats a foreground window with `WS_EX_TOOLWINDOW` or `WS_EX_NOACTIVATE` as part
  of the game and would keep reading keys. Tk's `Toplevel` with `-topmost` has neither (checked: extended style 0x108).
  Do not make the box an overlay or `overrideredirect` window for that reason.
- If the box ever fails to take focus, the keys are NOT blocked and typing would walk Link (WASD) and press buttons. It
  uses a synthetic Alt tap plus `SetForegroundWindow` to take focus, and the leading T that auto-repeats is swallowed.
  Watch for this the first time it is used in a game.
- Pad input still reaches the game while the box is open only if the user set `HOS_INPUT_BACKGROUND`.
- No bridge pause command is needed. (Longshot also has `hos_control_gate_swallow`, a bridge input gate, not used here.)

**Default: the external box (user, 22 Nov).** The in-game box was hard to read and did not put the cursor in the text field, so launches for the user pass `--env HOS_TBOX=0` (the in-game box off) and the external box above does the job.

## In-game box (Longshot `ootrr-tools` ce4a0ff)

The same T box drawn inside the game window by Longshot's own overlay, so there is no second window and no focus fight. It is the
default whenever the game has `HOS_NOTES_DIR` set; the external box above is the fallback (older builds, or `HOS_TBOX=0`).

- T (`HOS_TBOX_KEY`: a letter, a digit or F1 to F12) opens it when the game window is in front and the Insert menu is closed.
  Enter appends one line, Esc cancels, Backspace edits (append and delete at the end only; ASCII 0x20 to 0x7E, at most 200
  characters; Ctrl+V or Shift+Insert pastes the clipboard's first line, ASCII only). A green "note saved" line shows for 1.5 s.
- the last reply line from `notes\messages.jsonl` is shown above the typed text (re-read once a second while open).
- While it is open (and until Enter, Esc and the key are let go, at least 60 ms) the game gets no keyboard and no mouse: `game_in_front()`
  in `host\nn_hid.c` returns 0. Scripted and bridge input still count. Pads count only when `HOS_INPUT_BACKGROUND` is set (the
  same rule as for any window in front of the game); a pad stick held while typing would otherwise move Link only in that case.
- The line is the external box's shape: kind note, step null, text, verdict null, source `t-box`, `map` and `pos` from the game
  (left out when Link is not in play, for example during a load), shot null, time, t. No life or event fields.
- The game writes `notes\tbox-ingame.json` (its pid) when the box is on; `tbox.py` skips a foreground game with that pid, so
  both can run. `start.py` exports `HOS_NOTES_DIR=<session>\notes` (unless set) for the games the panel launches;
  `--tbox-ingame-only` does not start the external box at all.
- Needs the bridge (`HOS_CONTROL`): the box lives in the bridge's thread. Bridge command `tbox`: `op` state (default), open (the
  T key), type (`text`), key (`key` enter, esc or back), dir (`path`, the notes folder; for tests). Source: `host\control_tbox.inc`,
  drawing in `gpu\vk\vk_menu.cpp` `draw_tbox`, hooks in `host\display.c`, `MKWVK_API_VERSION` 38.
- If the tester binds the box key to a game button in `longshot_options.ini`, the game sees that key for the first frame or two.

### Test plan (in-game box)

Script, no window opened, no input to the game window: `python -I test_ingame_tbox.py --run <run folder> [--notes <temp folder>]`
against a game started with the bridge and `HOS_NOTES_DIR` (the panel's launch does both).
Expect ALL PASSED: state enabled, open, typing and Backspace, nothing written before Enter, one line with the right shape and
map and pos, Link's position unchanged, Esc writes nothing, empty Enter writes nothing.

By hand (a tester at the controls, in play, windowed; add to the capture playbook if it is not run):
1. Press T: a dark box appears at the bottom left (above any caption) with "NOTE", the last reply line and a blinking `> _`.
2. Type a sentence while holding nothing else; then try W, A, S, D, the arrow keys and Space while the box is open: the letters
   appear in the box, Link does not move, no menu opens. (Check also with the mouse: it does not turn the camera.)
3. Enter: "note saved" shows, the box closes, Link is controllable at once, the line is in `notes\notes.jsonl` with
   `"source": "t-box"` and the map and position. Esc during typing sends nothing and the game does not pause or open a menu.
4. Open the Insert menu with the box open (Insert key): the box closes. T with the Insert menu open does nothing.
5. Fullscreen (F11): the box still shows and works.
6. With the external box also running (panel started normally): T opens only the in-game one.
Not verified in a game when this was written: all of the above, and the box's look at other window sizes.

## Tested

Unit tests (`python -I test_tbox.py`): note append and format, state.json context and its staleness, last reply line,
ini rebind check, foreground check with Notepad standing in for the game (and the box's own process never counts).
Box style checked for tool-window and no-activate flags. NOT tested: the box itself against a running Longshot (the user
was playing; focus change would have interrupted them). First hands-on use is the test: press T, type, check the note
line appears, and that Link did not move while typing.

## Typing and focus (23 Nov fix)

- Keys typed between the T press and the box taking focus are caught by a low-level keyboard hook that is active only in
  that gap (at most 1.5 s, only while the game window was in front): they are kept from the game and typed into the box
  once it has focus, so the first letter is no longer lost. The held T that opened the box is ignored; a later t is typed.
- Enter and Esc are not given back to the game on key-down: focus returns to the game only after Enter, Esc and Shift
  are released, so the game never sees Enter held (it opened its menu before).
- The text is saved to `notes\tbox-draft.txt` on every key release, so a note cut short can be recovered.
- If the game window takes focus while the box is open, the box takes it back.
- Unit tests: `python -I panel\test_tbox_keys.py`.
