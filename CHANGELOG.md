# Changelog

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
