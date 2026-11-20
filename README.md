# Moonlet

A small, quiet layer of interface on top of Windows 10/11.

[English](#english) · [Русский](#русский)

![The orbit](docs/orbit.png)

## English

Moonlet puts an "orbit" at the top center of the screen: a small dark core
with satellites slowly circling around it, one satellite per feature. It is
written in Rust directly on Win32, Direct2D and DirectComposition — no web
views, no GUI frameworks.

- **Light.** One ~480 KB `moonlet.exe`, about 9 MB of RAM.
- **Quiet.** No polling: the process sleeps until Windows reports an event.
  Animations run inside DWM, so Moonlet itself uses 0% CPU while things move.
- **Kind to the battery.** Animations stop on battery power (can be turned
  off) and while a full-screen game or video is running. The integrated GPU
  is used, so a laptop's discrete GPU stays asleep.
- **Private.** No telemetry. The only network request is the daily update
  check, and it can be turned off.
- **Hands off the system.** No injection into `explorer.exe`, no patched
  system files. The taskbar, Start menu and Explorer are left alone.

### What it does

- **Satellites.** Clock (the ring fills over the hour), Music (the track's
  progress; works with Spotify, browsers and players that show up in the
  Windows media controls) and Battery (charge and state). Their color and
  ring show the state without opening anything.
- **Hover the core** and the satellites line up in fixed places with labels.
  Hover a satellite to see its card: date and time, battery details, or the
  current track with cover art, playback buttons and system volume. The mouse
  wheel over the music satellite changes the volume.
- **Click the core** to open the hub: screenshot of an area, pause the orbit,
  circle or oval orbit, settings.
- **Temporary satellites.** Plugging the charger in or out and a low battery
  (20% and 10%) send a small copy of the battery satellite out with a short
  note. A new song can do the same (off by default).

![A temporary satellite](docs/notice.png)
![The hub](docs/hub.png)

### Install

1. Download `moonlet.exe` from [Releases](https://github.com/BeautifulPanda/moonlet/releases)
   and put it in any folder you can write to (for example `%LOCALAPPDATA%\Moonlet`).
2. Run it. The moon icon appears in the tray; the menu has everything,
   including "Start with Windows".

The file is not code-signed, so Windows SmartScreen may warn about an unknown
publisher the first time.

### Settings

Tray menu → "Open settings" opens `%APPDATA%\Moonlet\config.toml`. Changes
apply as soon as the file is saved. Every option is commented in the file:
orbit shape and speed, which satellites to show and in what order, temporary
satellites, language (`auto`, `en`, `ru`) and updates.

### Updates

Moonlet checks GitHub a minute after start and then once a day. Modes, in the
tray menu or in `config.toml`:

- `auto` (default) — download, verify and install the new version, then restart.
  It waits while a full-screen app is running.
- `check` — only show a notification; clicking it installs the update.
- `off` — never connect to the network.

Every release is signed. Moonlet verifies the signature with a key built into
the program and refuses any file that does not match.

### Building

Requirements: Windows 10 x64, [Rust](https://rustup.rs) (MSVC toolchain),
Visual Studio Build Tools with the C++ workload.

```
cargo build --release
```

The result is `target\release\moonlet.exe`. Features can be left out at build
time, for example `--no-default-features --features orbit` builds the orbit
without music and updates.

### Plans

- Command line on a hotkey (apps, calculator, web search, Moonlet commands).
- Desktop widgets.
- Top bar and themes.
- Live wallpapers.
- Games: FPS counter (optional, off by default).

### License

[MIT](LICENSE)

## Русский

Moonlet добавляет поверх Windows небольшой тихий слой интерфейса — «орбиту»
у верхнего края экрана: тёмное ядро и медленно кружащие вокруг него спутники,
по одному на функцию. Написан на Rust прямо на Win32, Direct2D и
DirectComposition — без веб-движков и GUI-фреймворков.

- **Лёгкий.** Один `moonlet.exe` около 480 КБ, около 9 МБ памяти.
- **Тихий.** Никакого опроса: процесс спит, пока Windows не сообщит о событии.
  Анимации выполняет DWM, сам Moonlet в это время не тратит CPU.
- **Бережёт батарею.** Анимации останавливаются от батареи (можно отключить)
  и во время полноэкранных игр и видео. Работает на встроенной видеокарте —
  дискретная в ноутбуке не просыпается.
- **Приватный.** Никакой телеметрии. Единственный выход в сеть — проверка
  обновлений раз в сутки, её можно выключить.
- **Не лезет в систему.** Не внедряется в `explorer.exe` и не патчит системные
  файлы. Панель задач, «Пуск» и Проводник не трогает.

### Что умеет

- **Спутники.** Часы (кольцо заполняется за час), Музыка (прогресс трека;
  работает со Spotify, браузерами и плеерами, которые видны в медиа-панели
  Windows) и Батарея (заряд и состояние). Цвет и кольцо показывают состояние,
  ничего не раскрывая.
- **Наведение на ядро** — спутники встают на постоянные места с подписями.
  Наведение на спутник открывает карточку: дата и время, подробности о батарее
  или текущий трек с обложкой, кнопками и громкостью. Колёсико над спутником
  музыки меняет громкость.
- **Клик по ядру** — хаб: скриншот области, пауза орбиты, круг или овал,
  настройки.
- **Временные спутники.** Подключили или отключили зарядку, заряд упал до 20%
  и 10% — из спутника батареи вылетает его маленькая копия с короткой надписью.
  Так же можно показывать новый трек (по умолчанию выключено).

### Установка

1. Скачайте `moonlet.exe` в [Releases](https://github.com/BeautifulPanda/moonlet/releases)
   и положите в любую папку, куда у вас есть права на запись
   (например, `%LOCALAPPDATA%\Moonlet`).
2. Запустите. В трее появится значок-полумесяц; в его меню есть всё,
   включая «Запускать с Windows».

Файл не подписан сертификатом, поэтому при первом запуске SmartScreen может
предупредить о неизвестном издателе.

### Настройки

Меню трея → «Открыть настройки» открывает `%APPDATA%\Moonlet\config.toml`.
Изменения применяются сразу после сохранения. Каждый параметр описан прямо в
файле: форма и скорость орбиты, какие спутники и в каком порядке, временные
спутники, язык (`auto`, `ru`, `en`) и обновления.

### Обновления

Moonlet проверяет GitHub через минуту после запуска и дальше раз в сутки.
Режимы — в меню трея или в `config.toml`:

- `auto` (по умолчанию) — скачать, проверить и поставить новую версию, затем
  перезапуститься. Во время полноэкранных приложений ждёт.
- `check` — только сообщить; клик по уведомлению ставит обновление.
- `off` — вообще не выходить в сеть.

Каждый выпуск подписан. Moonlet проверяет подпись встроенным в программу
ключом и не ставит файл, который ей не соответствует.

### Сборка

Нужны: Windows 10 x64, [Rust](https://rustup.rs) (MSVC), Visual Studio Build
Tools с набором C++.

```
cargo build --release
```

Готовый файл — `target\release\moonlet.exe`. Части можно исключить при сборке,
например `--no-default-features --features orbit` соберёт орбиту без музыки и
обновлений.

### Планы

- Командная строка по горячей клавише (программы, калькулятор, поиск, команды).
- Виджеты на рабочем столе.
- Верхний бар и темы.
- Живые обои.
- Игры: счётчик FPS (по желанию, по умолчанию выключен).

### Лицензия

[MIT](LICENSE)
