# Changelog

## [0.7.0] - 2026-11-25

- The exe carries its own crescent icon (16 to 256 px, the same shape as the
  tray icon) and version info, so File Explorer, shortcuts and Task Manager
  show "Moonlet" with the moon instead of the default icon. `build.rs` writes
  the `.res` file itself; no rc.exe or new crate is needed. The exe grew by
  about 10 KB.
- Notes: the mode icon on the left of the strip is now a moon-colored button.
  A click sticks the note to the window under its middle, or to the desktop
  (under all windows) if there is none; another click unsticks it. Hovering
  the icon tells what a click will do. Right-click opens the mode menu
  (always on top, on the desktop, normal window). The `⋯` button and the
  "New note" and "Delete note" menu items are gone: use `✕`, the tray menu,
  the command line or the hub tile.
- Scenarios (Cargo feature `scenarios`, `[modules] scenarios`). `remember work`
  in the command line (Russian `запомнить …` too) saves the open windows to
  `scenarios\work.txt`: the program, where its window is, whether it is
  maximized or minimized, the File Explorer folder, the active tab of each
  browser window (read through UI Automation) and the app id of Store apps.
  Typing `work` then moves the open windows back and starts what is missing;
  new windows are caught with window events, not polling, for up to 30 s.
  Windows of apps running as administrator can't be moved, and the result
  says so. With several screens the command line offers "as saved" or a
  screen; windows are mapped proportionally to the chosen screen. Replacing
  an existing scenario needs a second Enter.
- A Scenarios section in the settings: save the open windows under a name,
  look at a scenario's windows, remove a window or a tab, add a tab, delete a
  scenario (second press). Hub tiles can now be `scenario:<name>`.
- Chrome and Edge lock their own session file while running, so tabs come
  from the address bar of each window (the active tab), not from the session.

## [0.6.1] - 2026-11-21

- Notes (Cargo feature `notes`, `[modules] notes`). A note is a small dark
  window with a plain text editor (the Windows RichEdit control): the Windows
  font, links that open with a click, Ctrl+wheel to zoom. Create one with
  `note buy milk` in the command line (Russian `заметка …` too), the `note`
  hub tile or "New note" in the tray menu; `show notes` brings them all
  forward. There is no limit on how many.
- The note's `⋯` menu (or a right click on it): always on top (hidden during
  full-screen apps), on the desktop under all windows, or a normal window.
  `[notes] mode` sets it for new notes.
- Sticking to a window: put the note over a program's window and pick
  "Stick to window: …". The note keeps its place from the nearest corner of
  that window, moves and hides with it and comes back when the program opens
  again (it is recognized by its exe). In a browser it sticks to the tab: it is
  shown only while the window's title is that tab's title. Window events come
  from `SetWinEventHook`; there are no hooks without stuck notes, and nothing
  is polled.
- `✕` on a note with text asks "Delete?" first and deletes on a second click
  within 3 seconds; an empty note closes at once.
- Each note is `%APPDATA%\Moonlet\notes\<number>.txt` (plain text, saved a
  second after the last edit); places and modes are in `layout.txt` there.
- Settings → Notes: the module switch, the mode of a new note, the list of all
  notes with Show and Delete (twice), and the folder.

## [0.6.0] - 2026-11-21

- Timers and reminders from the command line (Russian words work too):
  - `timer 10 min`, `timer 1 h 30 min tea`, `timer 1:30`;
  - `timer until 11:10` and `alarm 7:30`: a timer that ends at a clock time;
  - `remind me in 20 min call mom`, `remind me at 15:30 …`,
    `remind me tomorrow at 9 …`, `remind me to call the bank in an hour`;
    `tomorrow` without a time means 9:00.
  Enter starts it. Up to 20 are kept in `%APPDATA%\Moonlet\reminders.txt`, so
  they survive restarts and sleep. One Windows waitable timer wakes Moonlet
  exactly at the nearest one; nothing is polled. A reminder that came due while
  the computer slept or Moonlet was closed shows up at once, marked "Missed".
- Each of the three nearest timers is a small pink satellite on the orbit. It
  takes the middle of a gap between the regular satellites, so their places
  never move. Its ring shows the time left and is redrawn only when it gets a
  pixel shorter. Hover the core to see them in a row next to it with the time
  left; the card shows the details and cancels the timer.
- When time is up, a small copy flies out with a note and a soft two-note
  chime plays (synthesized in code, no sound file). The note stays until you
  touch the mouse or keyboard. Several at once show as one note with "(+N)".
  Without the orbit the note goes to the tray.
- Settings → Timers: the sound (chime, your own .wav, or silent), a play
  button, and the active timers and reminders with Cancel. In config.toml:
  `[timers] sound = "drop" | "off" | "path to .wav"`.

## [0.5.0] - 2026-11-21

- Bluetooth hub tile (`bluetooth` in `[hub] tiles`): a click turns Bluetooth on
  or off, holding the tile for half a second (or a right click) opens the
  Windows Bluetooth devices page. The icon is dimmed while Bluetooth is off.
  Without an adapter the tile is hidden. The radio is switched on a background
  thread, and Windows reports changes made elsewhere (Action Center).
- Command line:
  - files and folders from the Windows Search index (names that start with the
    text first, then names that contain it), shown under apps and commands with
    their folder on the right. Moonlet keeps no index of its own; the query runs
    on a background thread while you type;
  - the empty field shows recent files;
  - Enter opens a file or folder, Ctrl+Enter shows it in Explorer;
  - `bluetooth` turns it on or off, the next line opens the devices page;
  - system commands: `sleep`, `lock`, `shutdown`, `restart`, `empty bin`
    (with the number of items and the size). Shutting down, restarting and
    emptying the bin need a second Enter within 3 seconds, so a typo cannot
    trigger them. A hub command tile with such a command opens the command line
    with it waiting for Enter.

## [0.4.0] - 2026-11-20

- Quick access: the hub tiles are configurable. Settings → Quick access lists
  the satellites on the orbit and the hub tiles in order (arrows move a tile,
  the cross removes it) and adds new ones:
  - Moonlet actions: area screenshot, command line, music play/pause, next and
    previous track, rotation, orbit shape, settings;
  - your own tiles: an app (search the Start menu or pick any file), a file,
    a folder (Windows file dialog), a website, or a command line text that runs
    as if typed and confirmed with Enter (`volume 30`, `telegram`).
  - Apps and files show the first letter of their name, folders, sites and
    commands a matching icon. Up to 16 tiles; long names are shortened.
  - In `config.toml` the tiles are a list in `[hub]`:
    `tiles = ["screenshot", "app:C:\\Apps\\Telegram.lnk", "url:https://github.com|GitHub"]`.
- A satellite card is always drawn above the layout labels; before, the label
  of the next satellite could cover it.

## [0.3.1] - 2026-11-20

- "What's new" in the settings window (General → Updates): the changes of
  this version and the previous one, in the interface language.
- Clicking the "Moonlet updated" notification opens the settings on that list.

## [0.3.0] - 2026-11-20

- Settings window in Moonlet's style: tray menu → "Open settings", the hub
  tile or the `settings` command. Sections: General (language, start with
  Windows, pause on battery, updates and "Check now"), Orbit (on/off, shape,
  rotation, direction, speed, hover delay), Satellites (which ones and their
  order, temporary satellites and how long they stay), Command line (on/off,
  hotkey recorder that checks whether the combination is taken, web search
  engine or a custom address) and About.
  - Changes are saved to `config.toml` at once; comments in the file are kept.
    Hand edits made while the window is open show up in it, and an error in
    the file is shown with its line number.
  - Keyboard: Tab, arrows, Space and Enter; Ctrl+Tab switches sections;
    Esc closes the window.
  - The window is a separate process (`moonlet.exe --settings`): closing it
    returns all of its memory. Only one window opens at a time.
- The tray menu has "Open config.toml" for editing the file by hand.

## [0.2.0] - 2026-11-20

- Command line on `Ctrl+Alt+Space` (configurable in `config.toml`, section
  `[launcher]`): the field slides out of the orbit core with suggestions below.
  - Start menu apps, including names localized by Windows. A wrong keyboard
    layout and Russian typed for English names are understood
    (`ntktuhfv` → Telegram).
  - Moonlet commands in English and Russian: play/pause, next, previous,
    `volume N`, circle, oval, rotation, screenshot, settings.
  - Calculator with `+ - * / ^ %`, parentheses, `√` and a decimal comma;
    Enter copies the answer.
  - Site addresses open directly; anything else searches the web with the
    default browser's search engine (or an address set in `config.toml`).
- The tray menu shows the command line switch with its hotkey.
- The command line is not opened during full-screen apps. Its window and
  layers exist only while it is open.

## [0.1.0] - 2026-11-19

First public version.

- The orbit: a core at the top center of the screen with satellites circling
  it on a circular or oval orbit, with depth, glow and smooth transitions.
- Satellites: clock, music (Windows media sessions: track, cover, playback
  buttons, system volume, mouse wheel) and battery. Hidden when there is no
  battery.
- Hovering the core lays the satellites out with labels; hovering a satellite
  opens its card.
- Hub on click: area screenshot, pause, circle/oval, settings.
- Temporary satellites for charger and low battery events (and, optionally,
  a new song).
- Automatic pause on battery and in full-screen apps; recovers after GPU driver
  resets, display scale changes and Explorer restarts.
- `config.toml` with comments, applied on save; tray menu; start with Windows.
- Russian and English interface, chosen from the Windows language.
- Signed automatic updates from GitHub releases (auto, check only, off).
