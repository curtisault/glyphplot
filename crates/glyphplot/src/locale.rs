//! What a chart is told about the reader's locale and zone: the locale pack
//! (`spec/schemas/locale.schema.json`) and the zone table
//! (`spec/schemas/zone.schema.json`), which cross into the core as data
//! (ADR-112, REQ-I18N-020, REQ-TZ-020).
//!
//! The shell builds a pack from `Intl` and the CLI reads one from a file; the
//! core links no locale database and reads no zone or clock (REQ-TZ-010). A
//! pack is turned into the formatters' [`Locale`](crate::format::Locale) by
//! [`Locale::from_pack`](crate::format::Locale::from_pack).
//!
//! The default pack is en-US as the CLI writes it, with `-` for the minus; a
//! pack may set U+2212, as the browser's does (REQ-FMT-060). A zone table
//! becomes a time scale's [`Zone`](crate::time::Zone) through
//! [`ZoneTable::zone`]: UTC's stays d3's `scaleUtc`, any other draws local
//! time by the table as d3's `scaleTime` does under `TZ=<zone>`
//! (REQ-TZ-030). The pack's hour cycle picks 12- or 24-hour time labels
//! (REQ-FMT-050).

use std::fmt;

use serde::{Deserialize, Serialize};

/// Whether time labels are drawn on a 12- or 24-hour clock.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HourCycle {
    /// A 12-hour clock with the pack's `periods`.
    H12,
    /// A 24-hour clock.
    #[default]
    H23,
}

/// The text direction of a locale.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    /// Left to right.
    #[default]
    Ltr,
    /// Right to left.
    Rtl,
}

/// A locale pack in the shape of `locale.schema.json` (REQ-I18N-020): the
/// symbols, digits, names and patterns labels are formatted with.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalePack {
    /// The decimal separator.
    pub decimal: String,
    /// The group separator; empty for none. U+2009, U+202F and U+00A0 are
    /// drawn as one plain space cell (REQ-FMT-060).
    pub group: String,
    /// Group sizes from the decimal point leftward, the last repeating.
    pub grouping: Vec<u32>,
    /// The minus sign: `-` by default, U+2212 in the browser's packs
    /// (REQ-FMT-060).
    pub minus: String,
    /// The percent pattern, `#` standing for the number.
    pub percent: String,
    /// The ten digits.
    pub numerals: Vec<String>,
    /// The currency's prefix and suffix.
    pub currency: [String; 2],
    /// Month names, January first.
    pub months: Vec<String>,
    /// Short month names, January first.
    pub months_short: Vec<String>,
    /// Weekday names, Sunday first.
    pub days: Vec<String>,
    /// Short weekday names, Sunday first.
    pub days_short: Vec<String>,
    /// The AM and PM markers.
    pub periods: [String; 2],
    /// The date-and-time pattern.
    pub date_time: String,
    /// The date pattern.
    pub date: String,
    /// The time pattern.
    pub time: String,
    /// The hour cycle of time labels.
    pub hour_cycle: HourCycle,
    /// The text direction.
    pub direction: Direction,
}

/// A pack that does not fit `locale.schema.json`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocaleError(pub String);

impl fmt::Display for LocaleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "locale: {}", self.0)
    }
}

impl std::error::Error for LocaleError {}

/// Strings of an English list.
fn strings(names: &[&str]) -> Vec<String> {
    names.iter().map(|s| s.to_string()).collect()
}

impl Default for LocalePack {
    /// en-US with `-` for the minus, as the CLI and the core write without a
    /// pack (REQ-I18N-030, REQ-FMT-060).
    fn default() -> LocalePack {
        LocalePack {
            decimal: ".".into(),
            group: ",".into(),
            grouping: vec![3],
            minus: "-".into(),
            percent: "#%".into(),
            numerals: (0..10).map(|d| d.to_string()).collect(),
            currency: ["$".into(), String::new()],
            months: strings(&["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"]),
            months_short: strings(&["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]),
            days: strings(&["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"]),
            days_short: strings(&["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]),
            periods: ["AM".into(), "PM".into()],
            date_time: "%x, %X".into(),
            date: "%-m/%-d/%Y".into(),
            time: "%-I:%M:%S %p".into(),
            hour_cycle: HourCycle::H12,
            direction: Direction::Ltr,
        }
    }
}

impl LocalePack {
    /// A pack from its JSON, checked against `locale.schema.json`'s counts:
    /// ten numerals, twelve months, seven days, at least one group size of 1
    /// or more, a `#` in the percent pattern and no empty name.
    pub fn from_json(json: &str) -> Result<LocalePack, LocaleError> {
        let pack: LocalePack = serde_json::from_str(json).map_err(|e| LocaleError(e.to_string()))?;
        pack.check()?;
        Ok(pack)
    }

    /// Whether the pack keeps the schema's rules.
    pub fn check(&self) -> Result<(), LocaleError> {
        let count = |what: &str, v: &[String], n: usize| {
            if v.len() != n || v.iter().any(String::is_empty) {
                Err(LocaleError(format!("`{what}` takes {n} non-empty names, not {}", v.len())))
            } else {
                Ok(())
            }
        };
        count("numerals", &self.numerals, 10)?;
        count("months", &self.months, 12)?;
        count("monthsShort", &self.months_short, 12)?;
        count("days", &self.days, 7)?;
        count("daysShort", &self.days_short, 7)?;
        if self.decimal.is_empty() || self.minus.is_empty() {
            return Err(LocaleError("`decimal` and `minus` are not empty".into()));
        }
        if self.grouping.is_empty() || self.grouping.contains(&0) {
            return Err(LocaleError("`grouping` takes one or more sizes of 1 or more".into()));
        }
        if !self.percent.contains('#') {
            return Err(LocaleError("`percent` holds `#` for the number".into()));
        }
        Ok(())
    }

    /// The pack as JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// One change of offset: from `at` on, local time is UTC plus `offset`
/// minutes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transition {
    /// The instant the offset starts, in epoch ms.
    pub at: i64,
    /// Local time minus UTC, in minutes.
    pub offset: i32,
}

/// A zone's UTC offsets over a span, in the shape of `zone.schema.json`
/// (REQ-TZ-020); local-time ticks and labels read it and nothing else
/// (REQ-TZ-010, REQ-TZ-030).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoneTable {
    /// The zone as `timeZone` named it.
    pub name: String,
    /// Each change of offset, `at` strictly increasing.
    pub transitions: Vec<Transition>,
    /// The span the table is good for, in epoch ms.
    pub covering: [i64; 2],
}

impl Default for ZoneTable {
    /// UTC over every instant from the epoch to 2026-12-31 and before: one
    /// transition of offset 0.
    fn default() -> ZoneTable {
        ZoneTable { name: "UTC".into(), transitions: vec![Transition { at: 0, offset: 0 }], covering: [0, 1_798_761_600_000] }
    }
}

impl ZoneTable {
    /// A table from its JSON: at least one transition, sorted strictly by
    /// `at`, offsets within ±18 hours.
    pub fn from_json(json: &str) -> Result<ZoneTable, LocaleError> {
        let table: ZoneTable = serde_json::from_str(json).map_err(|e| LocaleError(e.to_string()))?;
        if table.name.is_empty() || table.transitions.is_empty() {
            return Err(LocaleError("a zone table has a name and at least one transition".into()));
        }
        if table.transitions.windows(2).any(|w| w[0].at >= w[1].at) {
            return Err(LocaleError("transitions are sorted by `at`, strictly increasing".into()));
        }
        if table.transitions.iter().any(|t| t.offset.abs() > 1080) {
            return Err(LocaleError("offsets are within ±1080 minutes".into()));
        }
        Ok(table)
    }

    /// The offset in minutes at the instant `t` (epoch ms): the last
    /// transition's at or before it, else the first's; 0 for a table with
    /// none.
    pub fn offset_at(&self, t: f64) -> i32 {
        let i = self.transitions.partition_point(|tr| (tr.at as f64) <= t);
        self.transitions.get(i.saturating_sub(1)).map_or(0, |tr| tr.offset)
    }

    /// Whether every offset is 0: the table is UTC's.
    pub fn is_utc(&self) -> bool {
        self.transitions.iter().all(|tr| tr.offset == 0)
    }

    /// The zone a time scale draws in by this table: [`Zone::Utc`] for a
    /// table whose offsets are all 0, so UTC keeps d3's `scaleUtc` ticks and
    /// labels; otherwise local time by the table (REQ-TZ-030).
    ///
    /// [`Zone::Utc`]: crate::time::Zone::Utc
    pub fn zone(&self) -> crate::time::Zone {
        if self.is_utc() {
            crate::time::Zone::Utc
        } else {
            crate::time::Zone::table(self.clone())
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
    fn schema_examples_read() {
        let de = LocalePack::from_json(&example("locale.json")).unwrap();
        assert_eq!((de.decimal.as_str(), de.minus.as_str(), de.months[2].as_str()), (",", "\u{2212}", "März"));
        let back = LocalePack::from_json(&de.to_json()).unwrap();
        assert_eq!(back, de);
        let ny = ZoneTable::from_json(&example("zone.json")).unwrap();
        assert_eq!((ny.offset_at(1_772_953_200_000.0), ny.offset_at(1_772_953_199_999.0)), (-240, -300));
        assert_eq!(ZoneTable::from_json(&example("zone.utc.json")).unwrap().offset_at(5.0), 0);
    }

    #[test]
    fn default_pack_keeps_the_schema_and_writes_hyphen_minus() {
        let pack = LocalePack::default();
        pack.check().unwrap();
        assert_eq!(pack.minus, "-");
        assert!(LocalePack::from_json(&pack.to_json()).is_ok());
        assert!(LocalePack::from_json(r#"{"decimal":"."}"#).is_err());
        let mut short = pack.clone();
        short.months.pop();
        assert!(short.check().is_err());
    }
}
