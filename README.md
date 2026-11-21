# gestus

[![ci](https://github.com/Redox-openDriver/gestus/actions/workflows/ci.yml/badge.svg)](https://github.com/Redox-openDriver/gestus/actions/workflows/ci.yml)
[![license](https://img.shields.io/badge/license-AGPL--3.0--or--later-blue)](LICENSE)

Touchpad gestures independent of the device and the operating system: pointer acceleration, taps and clicks by finger
count, two-finger scrolling, swipes, thumb and palm rejection tuned on real recordings — and a few dozen optional
features, each switched on in a TOML settings file: scroll inertia, corner taps and holds (a double tap that turns the
touchpad off), knock codes, three-finger drag, edge rails for volume and brightness, circular scrolling, rotation and
pinch, drawn symbols with an orientation-aware $1 recognizer, a numpad on the pad, a joystick corner, dwell click,
1€ smoothing, a tablet mode and more. A native mode leaves the usual gestures to the system (libinput on Linux) and
tells which contacts it may see. `no_std` with `alloc`; reading the settings is behind the `config` feature. It is the
gesture engine of [elantp-327e](https://github.com/Redox-openDriver/elantp-327e). The documentation below is in
Russian.

Библиотека жестов тачпада без привязки к устройству и ОС. Драйвер тачпада отвечает только за своё устройство: прочитать
касания и показать системе результат. Что из касаний получается — указатель, щелчки, прокрутка, свайпы и все
дополнительные жесты, — решает `gestus`, одинаково в Linux, Redox и где угодно ещё.

## Что внутри
| модуль | что делает |
|---|---|
| `engine` | `Gestures` — движок: касания в события; `feed`, `tick` и `deadline`; `typed` — нажата клавиша; родной режим `native` с `visible` и `hidden` |
| `settings` | `Config` и его разделы; всё дополнительное выключено, пока не включишь |
| `config` | разбор TOML (признак `config`): `parse`, `load`, свои символы рядом с настройками, ошибки со строкой и столбцом |
| `keys` | клавиши, сочетания, действия и ползунки |
| `symbols` | распознавание нарисованных символов: $1 с учётом ориентации, 17 встроенных и выученные свои |
| `inertia` | полёт прокрутки и указателя с трением |
| `filter` | фильтр 1€ против дрожания |
| `shaper` | события в целые шаги указателя, состояние кнопок и щелчки колеса |

## Подключение тачпада
```rust
use gestus::{Config, Event, Gestures, Input, Surface, Touch};

let mut gestures = Gestures::new(Config::default(), Surface { width: 117.0, height: 72.0 });
let events = gestures.feed(&Input { time, button, touches });
let wake = gestures.deadline();
let later = gestures.tick(now);
```

Время — микросекунды монотонных часов, положения касаний — миллиметры от левого верхнего угла площадки, `palm` —
отметка ладони от самого тачпада. На каждый отчёт тачпада — `feed`. Если `deadline` что-то вернул, драйвер будит
движок в этот момент через `tick`, даже когда пальцев нет: так работают полёт прокрутки, удержание угла, ожидание
второго тапа и джойстик.

События:

| событие | что с ним делать |
|---|---|
| `Motion`, `Scroll` | сдвинуть указатель, прокрутить; `Shaper` превращает их в целые шаги |
| `Press`, `Release` | кнопки мыши |
| `Key` | нажать или отпустить клавишу (`Key::name` — её имя) |
| `Swipe` | свайп с началом, продолжением и концом — если система хочет свои свайпы |
| `Pen`, `PenAway` | перо режима планшета: положение от 0 до 1 |
| `Touch` | только в родном режиме: повторить для системы тап, которого она не видела |
| `Notice` | сообщить человеку: тачпад выключен, цифровой блок включён, распознан символ…; ключ сообщения — `Notice::key`, все ключи — `Notice::KEYS` |

В родном режиме (`Gestures::native`) обычные жесты делает система, а движок после каждого `feed` и `tick` говорит, какие
касания ей показывать: `visible` и `hidden`.

## Настройки
Все разделы, параметры, действия и ползунки — в [docs/settings.md](docs/settings.md), пример с почти всем
включённым — [docs/example.toml](docs/example.toml).

## Лицензия

[GNU AGPL-3.0-or-later](LICENSE), © 2026 AristarkhKenebas.

Код написан с помощью ИИ-ассистента под руководством автора, поэтому в upstream Redox OS не отправляется: Redox не
принимает код, созданный языковыми моделями.
