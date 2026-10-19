//! Input events in cells, as `spec/schemas/input.schema.json` shapes them
//! (REQ-INPUT-020, ADR-115).
//!
//! The JSON form is canonical: tests, the contract, the CLI's scripted input
//! and recorded sessions use it ([`Input::from_json`]). The browser crosses
//! the boundary with numbers, `input(kind, x, y, a, b, buttons, mods, t)`,
//! which [`Input::decode`] reads exactly as the plan's "Input encoding"
//! table writes them, so the hot path builds no strings. Positions are in
//! cells from the grid's top left with fractions inside a cell; `t` is play
//! time in seconds, used only to stamp `motion` (the reducer reads no clock,
//! REQ-INPUT-010).

use serde::de::{self, Deserializer};
use serde::ser::{SerializeSeq, Serializer};
use serde::{Deserialize, Serialize};

/// The modifier keys held, as a bitmask: shift 1, ctrl 2, alt 4, meta 8.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Mods(pub u8);

impl Mods {
    /// No modifier.
    pub const NONE: Mods = Mods(0);
    /// Shift.
    pub const SHIFT: Mods = Mods(1);
    /// Control.
    pub const CTRL: Mods = Mods(2);
    /// Alt or Option.
    pub const ALT: Mods = Mods(4);
    /// Meta: ⌘ or the Windows key.
    pub const META: Mods = Mods(8);
    const NAMES: [(&'static str, u8); 4] = [("shift", 1), ("ctrl", 2), ("alt", 4), ("meta", 8)];

    /// Whether shift is held.
    pub fn shift(self) -> bool {
        self.0 & 1 != 0
    }

    /// Whether ctrl or meta is held: what makes the wheel zoom
    /// (REQ-INPUT-040).
    pub fn ctrl_or_meta(self) -> bool {
        self.0 & (2 | 8) != 0
    }
}

impl Serialize for Mods {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let names: Vec<&str> = Mods::NAMES.iter().filter(|(_, b)| self.0 & b != 0).map(|(n, _)| *n).collect();
        let mut seq = s.serialize_seq(Some(names.len()))?;
        for n in names {
            seq.serialize_element(n)?;
        }
        seq.end()
    }
}

impl<'de> Deserialize<'de> for Mods {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Mods, D::Error> {
        let names = Vec::<String>::deserialize(d)?;
        let mut bits = 0;
        for n in names {
            let (_, b) = Mods::NAMES.iter().find(|(name, _)| *name == n).ok_or_else(|| de::Error::custom(format!("`{n}` is not shift, ctrl, alt or meta")))?;
            bits |= b;
        }
        Ok(Mods(bits))
    }
}

/// The pointer's type, as `PointerEvent.pointerType`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PointerType {
    /// A mouse (the default).
    #[default]
    Mouse,
    /// A pen.
    Pen,
    /// A finger.
    Touch,
}

/// The keys the shell sends, by the plan's key codes; any other key is
/// [`Key::Other`], which the reducer ignores.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    /// `ArrowLeft`, code 0.
    Left,
    /// `ArrowRight`, code 1.
    Right,
    /// `ArrowUp`, code 2.
    Up,
    /// `ArrowDown`, code 3.
    Down,
    /// `Home`, code 4.
    Home,
    /// `End`, code 5.
    End,
    /// `PageUp`, code 6.
    PageUp,
    /// `PageDown`, code 7.
    PageDown,
    /// `+`, code 8.
    Plus,
    /// `=`, code 9.
    Equals,
    /// `-`, code 10.
    Minus,
    /// `0`, code 11.
    Zero,
    /// `Enter`, code 12.
    Enter,
    /// `Escape`, code 13.
    Escape,
    /// `" "`, code 14.
    Space,
    /// `?`, code 15.
    Question,
    /// `1` to `9`, codes 16 to 24.
    Digit(u8),
    /// The CLI's `h`, code 25.
    H,
    /// The CLI's `j`, code 26.
    J,
    /// The CLI's `k`, code 27.
    K,
    /// The CLI's `l`, code 28.
    L,
    /// The CLI's `f`, code 29.
    F,
    /// The CLI's `q`, code 30.
    Q,
    /// Any other key.
    Other,
}

impl Key {
    /// The key of a code of the plan's list, `None` past it.
    pub fn from_code(code: u32) -> Option<Key> {
        Some(match code {
            0 => Key::Left,
            1 => Key::Right,
            2 => Key::Up,
            3 => Key::Down,
            4 => Key::Home,
            5 => Key::End,
            6 => Key::PageUp,
            7 => Key::PageDown,
            8 => Key::Plus,
            9 => Key::Equals,
            10 => Key::Minus,
            11 => Key::Zero,
            12 => Key::Enter,
            13 => Key::Escape,
            14 => Key::Space,
            15 => Key::Question,
            16..=24 => Key::Digit((code - 15) as u8),
            25 => Key::H,
            26 => Key::J,
            27 => Key::K,
            28 => Key::L,
            29 => Key::F,
            30 => Key::Q,
            _ => return None,
        })
    }

    /// The key of a `KeyboardEvent.key` value; [`Key::Other`] for one the
    /// shell does not send.
    pub fn from_name(name: &str) -> Key {
        match name {
            "ArrowLeft" => Key::Left,
            "ArrowRight" => Key::Right,
            "ArrowUp" => Key::Up,
            "ArrowDown" => Key::Down,
            "Home" => Key::Home,
            "End" => Key::End,
            "PageUp" => Key::PageUp,
            "PageDown" => Key::PageDown,
            "+" => Key::Plus,
            "=" => Key::Equals,
            "-" => Key::Minus,
            "0" => Key::Zero,
            "Enter" => Key::Enter,
            "Escape" => Key::Escape,
            " " => Key::Space,
            "?" => Key::Question,
            "h" => Key::H,
            "j" => Key::J,
            "k" => Key::K,
            "l" => Key::L,
            "f" => Key::F,
            "q" => Key::Q,
            d if d.len() == 1 && (b'1'..=b'9').contains(&d.as_bytes()[0]) => Key::Digit(d.as_bytes()[0] - b'0'),
            _ => Key::Other,
        }
    }

    /// The key's `KeyboardEvent.key` value.
    pub fn name(self) -> &'static str {
        const DIGITS: [&str; 9] = ["1", "2", "3", "4", "5", "6", "7", "8", "9"];
        match self {
            Key::Left => "ArrowLeft",
            Key::Right => "ArrowRight",
            Key::Up => "ArrowUp",
            Key::Down => "ArrowDown",
            Key::Home => "Home",
            Key::End => "End",
            Key::PageUp => "PageUp",
            Key::PageDown => "PageDown",
            Key::Plus => "+",
            Key::Equals => "=",
            Key::Minus => "-",
            Key::Zero => "0",
            Key::Enter => "Enter",
            Key::Escape => "Escape",
            Key::Space => " ",
            Key::Question => "?",
            Key::Digit(d) => DIGITS[usize::from(d.clamp(1, 9) - 1)],
            Key::H => "h",
            Key::J => "j",
            Key::K => "k",
            Key::L => "l",
            Key::F => "f",
            Key::Q => "q",
            Key::Other => "Unidentified",
        }
    }
}

impl Serialize for Key {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.name())
    }
}

impl<'de> Deserialize<'de> for Key {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Key, D::Error> {
        let name = String::deserialize(d)?;
        if name.is_empty() {
            return Err(de::Error::custom("a key is not empty"));
        }
        Ok(Key::from_name(&name))
    }
}

fn one() -> u8 {
    1
}

/// One input event, discriminated by `kind` (input.schema.json). Positions
/// are in cells with fractions; `t` is play time in seconds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum Input {
    /// The pointer moved.
    Move {
        /// Play time.
        t: f64,
        /// Column.
        x: f64,
        /// Row.
        y: f64,
        /// The pointer's type.
        #[serde(default)]
        pointer: PointerType,
        /// Buttons held, as `PointerEvent.buttons`.
        #[serde(default)]
        buttons: u8,
        /// Modifiers held.
        #[serde(default)]
        mods: Mods,
    },
    /// A button went down.
    Down {
        /// Play time.
        t: f64,
        /// Column.
        x: f64,
        /// Row.
        y: f64,
        /// The pointer's type.
        #[serde(default)]
        pointer: PointerType,
        /// Buttons held.
        #[serde(default)]
        buttons: u8,
        /// Modifiers held.
        #[serde(default)]
        mods: Mods,
    },
    /// A button went up.
    Up {
        /// Play time.
        t: f64,
        /// Column.
        x: f64,
        /// Row.
        y: f64,
        /// The pointer's type.
        #[serde(default)]
        pointer: PointerType,
        /// Buttons still held.
        #[serde(default)]
        buttons: u8,
        /// Modifiers held.
        #[serde(default)]
        mods: Mods,
    },
    /// The pointer left the chart or the chart lost focus.
    Leave {
        /// Play time.
        t: f64,
    },
    /// Wheel deltas summed over an animation frame, in lines. A wheel of no
    /// delta ends a wheel gesture (`plot-view` with `final`).
    Wheel {
        /// Play time.
        t: f64,
        /// Column of the pointer.
        x: f64,
        /// Row of the pointer.
        y: f64,
        /// Horizontal delta.
        dx: f64,
        /// Vertical delta; negative is up.
        dy: f64,
        /// Modifiers held.
        #[serde(default)]
        mods: Mods,
    },
    /// A two-pointer pinch or a trackpad pinch.
    Pinch {
        /// Play time.
        t: f64,
        /// Column of its centre.
        x: f64,
        /// Row of its centre.
        y: f64,
        /// The change in spread since the last pinch event; above 1 zooms in.
        scale: f64,
        /// True on its last event.
        #[serde(default)]
        end: bool,
    },
    /// A two-finger pan.
    Pan {
        /// Play time.
        t: f64,
        /// Horizontal movement in cells since the last pan event.
        dx: f64,
        /// Vertical movement in cells.
        dy: f64,
        /// True on its last event.
        #[serde(default)]
        end: bool,
        /// Horizontal velocity in cells a second at the end.
        #[serde(default)]
        velocity: f64,
    },
    /// A tap or a double tap (a double click from a mouse).
    Tap {
        /// Play time.
        t: f64,
        /// Column.
        x: f64,
        /// Row.
        y: f64,
        /// 1 for a tap, 2 for a double tap.
        #[serde(default = "one")]
        count: u8,
        /// The pointer that tapped, so a double-click's reset names the
        /// mouse.
        #[serde(default)]
        pointer: PointerType,
    },
    /// A long press, which starts a brush on touch.
    Press {
        /// Play time.
        t: f64,
        /// Column.
        x: f64,
        /// Row.
        y: f64,
    },
    /// A key pressed while the chart has focus.
    Key {
        /// Play time.
        t: f64,
        /// The key.
        key: Key,
        /// Modifiers held.
        #[serde(default)]
        mods: Mods,
    },
}

/// One input or a list of them.
#[derive(Deserialize)]
#[serde(untagged)]
enum OneOrMany {
    One(Input),
    Many(Vec<Input>),
}

impl Input {
    /// The kind codes of the plan's "Input encoding" table.
    pub const MOVE: u32 = 0;
    /// `down`.
    pub const DOWN: u32 = 1;
    /// `up`.
    pub const UP: u32 = 2;
    /// `leave`.
    pub const LEAVE: u32 = 3;
    /// `wheel`.
    pub const WHEEL: u32 = 4;
    /// `pinch`.
    pub const PINCH: u32 = 5;
    /// `pan`.
    pub const PAN: u32 = 6;
    /// `tap`.
    pub const TAP: u32 = 7;
    /// `press`.
    pub const PRESS: u32 = 8;
    /// `key`.
    pub const KEY: u32 = 9;

    /// One event from the boundary's numbers, as the plan's "Input encoding"
    /// table writes them (REQ-INPUT-020):
    ///
    /// | kind | code | x, y | a | b | buttons |
    /// |---|---|---|---|---|---|
    /// | move, down, up | 0, 1, 2 | position | 0 | 0 | buttons + 256 × pointer (mouse 0, pen 1, touch 2) |
    /// | leave | 3 | 0 | 0 | 0 | 0 |
    /// | wheel | 4 | position | dx | dy | 0 |
    /// | pinch | 5 | centre | scale | end (1 or 0) | 0 |
    /// | pan | 6 | velocity, end (1 or 0) | dx | dy | 0 |
    /// | tap | 7 | position | count (1 or 2) | 0 | 256 × pointer (mouse 0, pen 1, touch 2) |
    /// | press | 8 | position | 0 | 0 | 0 |
    /// | key | 9 | 0 | key code | 0 | 0 |
    ///
    /// `mods` is shift 1, ctrl 2, alt 4, meta 8. `None` for a kind or key
    /// code past the table, which the caller ignores.
    #[allow(clippy::too_many_arguments)]
    pub fn decode(kind: u32, x: f64, y: f64, a: f64, b: f64, buttons: u32, mods: u32, t: f64) -> Option<Input> {
        let mods = Mods((mods & 0xF) as u8);
        let pointer = match buttons >> 8 {
            1 => PointerType::Pen,
            2 => PointerType::Touch,
            _ => PointerType::Mouse,
        };
        let held = (buttons & 0x1F) as u8;
        Some(match kind {
            Input::MOVE => Input::Move { t, x, y, pointer, buttons: held, mods },
            Input::DOWN => Input::Down { t, x, y, pointer, buttons: held, mods },
            Input::UP => Input::Up { t, x, y, pointer, buttons: held, mods },
            Input::LEAVE => Input::Leave { t },
            Input::WHEEL => Input::Wheel { t, x, y, dx: a, dy: b, mods },
            Input::PINCH => Input::Pinch { t, x, y, scale: a, end: b != 0.0 },
            Input::PAN => Input::Pan { t, dx: a, dy: b, end: y != 0.0, velocity: x },
            Input::TAP => Input::Tap { t, x, y, count: if a >= 2.0 { 2 } else { 1 }, pointer },
            Input::PRESS => Input::Press { t, x, y },
            Input::KEY => Input::Key { t, key: Key::from_code(a as u32).filter(|_| a >= 0.0)?, mods },
            _ => return None,
        })
    }

    /// One event or a list of them from their JSON (input.schema.json).
    pub fn from_json(json: &str) -> Result<Vec<Input>, String> {
        match serde_json::from_str::<OneOrMany>(json) {
            Ok(OneOrMany::One(i)) => Ok(vec![i]),
            Ok(OneOrMany::Many(v)) => Ok(v),
            Err(e) => Err(format!("input: {e}")),
        }
    }

    /// The event as JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Its play time.
    pub fn t(&self) -> f64 {
        match *self {
            Input::Move { t, .. }
            | Input::Down { t, .. }
            | Input::Up { t, .. }
            | Input::Leave { t }
            | Input::Wheel { t, .. }
            | Input::Pinch { t, .. }
            | Input::Pan { t, .. }
            | Input::Tap { t, .. }
            | Input::Press { t, .. }
            | Input::Key { t, .. } => t,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example(name: &str) -> String {
        std::fs::read_to_string(format!("{}/../../spec/schemas/examples/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    #[test]
    fn schema_examples_read_and_round_trip() {
        let all = Input::from_json(&example("input.json")).unwrap();
        assert_eq!(all.len(), 10);
        assert_eq!(all[8], Input::Key { t: 4.5, key: Key::Right, mods: Mods::SHIFT });
        for i in &all {
            assert_eq!(&Input::from_json(&i.to_json()).unwrap()[0], i);
        }
        assert_eq!(Input::from_json(&example("input.key.json")).unwrap()[0], Input::Key { t: 0.0, key: Key::End, mods: Mods::NONE });
        assert!(matches!(Input::from_json(&example("input.move.json")).unwrap()[0], Input::Move { x, .. } if x == 0.5));
        assert!(Input::from_json(r#"{"t":0,"kind":"move","x":1}"#).is_err());
    }

    /// REQ-INPUT-020: the numeric encoding decodes to the events the JSON
    /// form names, field for field.
    #[test]
    fn decode_follows_the_encoding_table() {
        let all = Input::from_json(&example("input.json")).unwrap();
        let decoded = [
            Input::decode(0, 41.37, 10.62, 0.0, 0.0, 0, 0, 3.2),
            Input::decode(1, 41.4, 10.62, 0.0, 0.0, 1 + 512, 0, 3.25),
            Input::decode(2, 47.1, 10.8, 0.0, 0.0, 512, 0, 3.4),
            Input::decode(4, 41.37, 10.62, 0.0, -1.5, 0, 2, 3.6),
            Input::decode(5, 40.0, 12.0, 1.25, 0.0, 0, 0, 3.8),
            Input::decode(6, -18.0, 1.0, -2.5, 0.0, 0, 0, 3.9),
            Input::decode(7, 41.4, 10.62, 2.0, 0.0, 0, 0, 4.1),
            Input::decode(8, 41.4, 10.62, 0.0, 0.0, 0, 0, 4.3),
            Input::decode(9, 0.0, 0.0, 1.0, 0.0, 0, 1, 4.5),
            Input::decode(3, 0.0, 0.0, 0.0, 0.0, 0, 0, 4.7),
        ];
        for (d, j) in decoded.iter().zip(&all) {
            assert_eq!(d.as_ref(), Some(j));
        }
        for code in 0..31 {
            let k = Key::from_code(code).unwrap();
            assert_eq!(Key::from_name(k.name()), k, "code {code}");
        }
        assert_eq!(
            Input::decode(7, 1.0, 2.0, 2.0, 0.0, 512, 0, 0.0),
            Some(Input::Tap { t: 0.0, x: 1.0, y: 2.0, count: 2, pointer: PointerType::Touch }),
            "a tap's pointer is 256 × its type"
        );
        assert_eq!(Input::decode(7, 1.0, 2.0, 1.0, 0.0, 256, 0, 0.0), Some(Input::Tap { t: 0.0, x: 1.0, y: 2.0, count: 1, pointer: PointerType::Pen }));
        assert_eq!(Input::decode(9, 0.0, 0.0, 31.0, 0.0, 0, 0, 0.0), None);
        assert_eq!(Input::decode(10, 0.0, 0.0, 0.0, 0.0, 0, 0, 0.0), None);
    }
}
