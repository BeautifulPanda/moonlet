# Changelog

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
