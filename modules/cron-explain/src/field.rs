//! One cron field: parsing to a bitmask of allowed values, and describing it.

pub struct Spec {
    pub label: &'static str,
    pub min: u32,
    pub max: u32,
    pub names: &'static [&'static str],
}

pub const MONTHS: &[&str] = &["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
pub const DAYS: &[&str] = &["sun", "mon", "tue", "wed", "thu", "fri", "sat"];
const MONTH_NAMES: [&str; 12] = [
    "January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December",
];
const DAY_NAMES: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

pub const FIELDS: [Spec; 5] = [
    Spec { label: "minute", min: 0, max: 59, names: &[] },
    Spec { label: "hour", min: 0, max: 23, names: &[] },
    Spec { label: "day of month", min: 1, max: 31, names: &[] },
    Spec { label: "month", min: 1, max: 12, names: MONTHS },
    Spec { label: "day of week", min: 0, max: 7, names: DAYS },
];

pub struct Field {
    pub bits: u64,
    pub wildcard: bool,
    /// Cron counts any field written with a leading `*` as unrestricted when it picks between the day fields.
    pub starred: bool,
}

impl Field {
    pub fn has(&self, v: u32) -> bool {
        self.bits >> v & 1 == 1
    }

    pub fn values(&self) -> Vec<u32> {
        (0..64).filter(|v| self.has(*v)).collect()
    }
}

pub fn parse(text: &str, spec: &Spec) -> Result<Field, String> {
    let mut bits = 0u64;
    for part in text.split(',') {
        bits |= parse_part(part, spec).map_err(|e| format!("{} field {text:?}: {e}", spec.label))?;
    }
    if spec.max == 7 && bits >> 7 & 1 == 1 {
        bits = bits & !(1 << 7) | 1;
    }
    Ok(Field { bits, wildcard: text == "*", starred: text.starts_with('*') })
}

fn parse_part(part: &str, spec: &Spec) -> Result<u64, String> {
    let (range, step) = match part.split_once('/') {
        Some((r, s)) => (r, s.parse::<u32>().ok().filter(|s| *s > 0).ok_or("step must be a positive number")?),
        None => (part, 1),
    };
    let (lo, hi) = match range {
        "*" => (spec.min, spec.max),
        _ => match range.split_once('-') {
            Some((a, b)) => (value(a, spec)?, value(b, spec)?),
            None if step > 1 => (value(range, spec)?, spec.max),
            None => {
                let v = value(range, spec)?;
                (v, v)
            }
        },
    };
    if lo > hi {
        return Err(format!("{lo}-{hi} runs backwards"));
    }
    Ok((lo..=hi).step_by(step as usize).fold(0, |bits, v| bits | 1 << v))
}

fn value(text: &str, spec: &Spec) -> Result<u32, String> {
    let lower = text.to_ascii_lowercase();
    let named = spec.names.iter().position(|n| *n == lower).map(|i| i as u32 + spec.min);
    let v = match named {
        Some(v) => v,
        None => text.parse::<u32>().map_err(|_| format!("{text:?} is not a number or name"))?,
    };
    if v < spec.min || v > spec.max {
        return Err(format!("{v} is outside {}-{}", spec.min, spec.max));
    }
    Ok(v)
}

pub fn describe(field: &Field, spec: &Spec) -> String {
    let values = field.values();
    if field.wildcard {
        return format!("every {}", spec.label);
    }
    let full = (spec.min..=spec.max.min(if spec.max == 7 { 6 } else { spec.max })).count();
    if values.len() == full {
        return format!("every {}", spec.label);
    }
    let names: Vec<String> = values.iter().map(|v| name_of(*v, spec)).collect();
    if names.len() > 2 && values.windows(2).all(|w| w[1] == w[0] + 1) {
        return format!("{} through {}", names[0], names[names.len() - 1]);
    }
    names.join(", ")
}

fn name_of(v: u32, spec: &Spec) -> String {
    match spec.label {
        "month" => MONTH_NAMES[v as usize - 1].to_string(),
        "day of week" => DAY_NAMES[v as usize % 7].to_string(),
        _ => v.to_string(),
    }
}
