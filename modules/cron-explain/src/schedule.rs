//! Calendar arithmetic (UTC, proleptic Gregorian) and the next-run search.

use crate::field::Field;

const MAX_DAYS_SEARCHED: i64 = 366 * 8;

pub struct Stamp {
    pub year: i64,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
}

impl Stamp {
    pub fn show(&self) -> String {
        format!("{:04}-{:02}-{:02} {:02}:{:02}", self.year, self.month, self.day, self.hour, self.minute)
    }
}

pub struct Cron {
    pub minute: Field,
    pub hour: Field,
    pub dom: Field,
    pub month: Field,
    pub dow: Field,
}

pub fn parse_stamp(text: &str) -> Result<Stamp, String> {
    let bad = || format!("{text:?} is not a time, write it as 2026-01-31 08:30");
    let (date, time) = text.trim().split_once(' ').ok_or_else(bad)?;
    let d: Vec<i64> = date.split('-').map(|p| p.parse().map_err(|_| bad())).collect::<Result<_, _>>()?;
    let t: Vec<u32> = time.split(':').map(|p| p.parse().map_err(|_| bad())).collect::<Result<_, _>>()?;
    match (d.as_slice(), t.as_slice()) {
        ([y, mo, da], [h, mi]) if (1..=12).contains(mo) && *da >= 1 && *da <= days_in_month(*y, *mo as u32) as i64 && *h < 24 && *mi < 60 => {
            Ok(Stamp { year: *y, month: *mo as u32, day: *da as u32, hour: *h, minute: *mi })
        }
        _ => Err(bad()),
    }
}

/// Days since 1970-01-01 for a civil date (Howard Hinnant's algorithm).
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

fn days_in_month(y: i64, m: u32) -> u32 {
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    match m {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

impl Cron {
    /// Standard cron: either day field matching is enough only when neither is written with a leading `*`.
    fn day_matches(&self, m: u32, d: u32, days: i64) -> bool {
        if !self.month.has(m) {
            return false;
        }
        let weekday = (days + 4).rem_euclid(7) as u32;
        let by_month_day = self.dom.has(d);
        let by_weekday = self.dow.has(weekday);
        if self.dom.starred || self.dow.starred {
            by_month_day && by_weekday
        } else {
            by_month_day || by_weekday
        }
    }

    pub fn next_runs(&self, from: &Stamp, count: usize) -> Vec<Stamp> {
        let start = days_from_civil(from.year, from.month, from.day);
        let after = from.hour * 60 + from.minute;
        let mut runs = Vec::new();
        for offset in 0..MAX_DAYS_SEARCHED {
            let days = start + offset;
            let (y, m, d) = civil_from_days(days);
            if !self.day_matches(m, d, days) {
                continue;
            }
            for h in self.hour.values().into_iter().filter(|h| *h < 24) {
                for mi in self.minute.values().into_iter().filter(|mi| *mi < 60) {
                    if offset == 0 && h * 60 + mi <= after {
                        continue;
                    }
                    runs.push(Stamp { year: y, month: m, day: d, hour: h, minute: mi });
                    if runs.len() == count {
                        return runs;
                    }
                }
            }
        }
        runs
    }
}
