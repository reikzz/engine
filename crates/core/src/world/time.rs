use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TimeTick {
    Hour,
    Day,
    Month(u8),
    Year,
}

impl TimeTick {
    #[inline]
    pub fn is_new_day(self) -> bool {
        self >= TimeTick::Day
    }

    #[inline]
    pub fn is_new_month(self) -> bool {
        matches!(self, TimeTick::Month(_) | TimeTick::Year)
    }

    #[inline]
    pub fn is_new_year(self) -> bool {
        self == TimeTick::Year
    }

    #[inline]
    pub fn is_new_quarter(self) -> bool {
        matches!(self.month(), Some(1 | 4 | 7 | 10))
    }

    #[inline]
    pub fn month(self) -> Option<u8> {
        match self {
            TimeTick::Month(m) => Some(m),
            TimeTick::Year => Some(1),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct GameDate {
    year: u16,
    month: u8,
    day: u8,
    hour: u8,
}

impl Default for GameDate {
    fn default() -> Self {
        Self {
            year: 2020,
            month: 1,
            day: 1,
            hour: 0,
        }
    }
}

impl GameDate {
    pub(crate) fn new(year: u16, month: u8, day: u8, hour: u8) -> Option<Self> {
        if (1..=12).contains(&month) && (1..=30).contains(&day) && hour < 24 {
            Some(Self {
                year,
                month,
                day,
                hour,
            })
        } else {
            None
        }
    }

    #[inline]
    pub fn year(self) -> u16 {
        self.year
    }
    #[inline]
    pub fn month(self) -> u8 {
        self.month
    }
    #[inline]
    pub fn day(self) -> u8 {
        self.day
    }
    #[inline]
    pub fn hour(self) -> u8 {
        self.hour
    }

    #[inline]
    pub fn total_days(&self) -> u64 {
        self.year as u64 * 360 + (self.month as u64 - 1) * 30 + (self.day as u64 - 1)
    }

    pub fn add_days(&self, days: u32) -> Self {
        let new_total = self.total_days() + days as u64;
        let year = (new_total / 360) as u16;
        let rem = new_total % 360;
        let month = (rem / 30) as u8 + 1;
        let day = (rem % 30) as u8 + 1;
        Self {
            year,
            month,
            day,
            hour: self.hour,
        }
    }

    pub fn advance(&mut self) -> TimeTick {
        self.hour += 1;
        if self.hour < 24 {
            return TimeTick::Hour;
        }

        self.advance_day()
    }

    pub fn advance_day(&mut self) -> TimeTick {
        self.hour = 0;
        self.day += 1;
        if self.day <= 30 {
            return TimeTick::Day;
        }

        self.day = 1;
        self.month += 1;
        if self.month <= 12 {
            return TimeTick::Month(self.month);
        }

        self.month = 1;
        self.year = self.year.saturating_add(1);
        TimeTick::Year
    }
}

impl fmt::Display for GameDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:04}-{:02}-{:02} {:02}:00",
            self.year, self.month, self.day, self.hour
        )
    }
}
