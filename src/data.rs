use anyhow::{bail, ensure, Result};
use chrono::{
    DateTime, Datelike, Duration, Local, LocalResult, NaiveDate, NaiveDateTime, TimeZone, Utc,
    Weekday,
};
use regex::Regex;
use std::cmp::Ordering;
use std::fmt::{Display, Formatter};
use std::fs::{read_dir, File};
use std::io;
use std::io::BufRead;
use std::ops::Add;
use std::path::Path;
use std::str::FromStr;

use nom::branch::alt;
use nom::bytes::complete::{tag, take_while1};
use nom::character::complete::{alphanumeric0, char as nom_char, char};
use nom::combinator::{map, map_opt, map_res};
use nom::multi::separated_list0;
use nom::sequence::{delimited, preceded, separated_pair};
use nom::IResult as NomResult;

fn parse_tags(text: &str) -> NomResult<&str, Vec<&str>> {
    let sep = ' ';
    let quote = '"';
    let x = separated_list0(
        nom_char(sep),
        alt((
            delimited(
                nom_char(quote),
                take_while1(|c| c != quote),
                nom_char(quote),
            ),
            take_while1(|c| c != quote && c != sep),
        )),
    )(text);

    x
}

fn parse_date(input: &str) -> NomResult<&str, DateTime<Utc>> {
    map_opt(take_while1(|c| c != ' '), |v: &str| {
        let parsed = NaiveDateTime::parse_from_str(v.trim(), "%Y%m%dT%H%M%SZ");
        return if let Ok(d) = parsed {
            Some(DateTime::<Utc>::from_utc(d, Utc))
        } else {
            None
        };
    })(input)
}

/*
Parse a range.
Ranges can have multiple formats:
 <datetime> - <datetime>
 <datetime>
 :<period>
where:
  datetime is in the format "%Y%m%dT%H%M%SZ" (e.g.: 20220711T133312Z)
  period is one of
   - today
   - yesterday
   - week
   - lastweek
   - month
   - lastmonth
 */
fn parse_range(input: &str) -> NomResult<&str, Range> {
    alt((
        map_res(
            separated_pair(parse_date, tag(" - "), parse_date),
            |(from, to)| Range::new(from, Some(to)),
        ),
        map_res(parse_date, |r| Range::new(r, None)),
        preceded(char(':'), map_res(alphanumeric0, Range::from_period_str)),
    ))(input)
}

fn parse_entry(input: &str) -> NomResult<&str, TimeEntry> {
    preceded(
        tag("inc "),
        map(separated_pair(parse_range, tag(" # "), parse_tags), |t| {
            TimeEntry {
                range: t.0,
                tags: t
                    .1
                    .into_iter()
                    .map(|s| s.to_string().clone())
                    .collect::<Vec<String>>(),
                id: 0,
            }
        }),
    )(input)
}
/// Specify an optionally opened range of time. Times are stored in UTC and expected to be given in
/// UTC time.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Range {
    from: DateTime<Utc>,
    to: Option<DateTime<Utc>>,
}

impl Range {
    fn from_period_str(period: &str) -> Result<Range> {
        match period {
            "today" => Range::today(),
            "yesterday" => Range::yesterday(),
            "week" => Range::current_week(),
            "lastweek" => Range::last_week(),
            "month" => Range::current_month(),
            "lastmonth" => Range::last_month(),
            _ => bail!(""),
        }
    }

    /// Print the duration in a HH:MM:SS format
    pub fn pretty_duration(d: &Duration) -> String {
        format!(
            "{:02}:{:02}:{:02}",
            d.num_hours(),
            d.num_minutes() % 60,
            d.num_seconds() % 60
        )
    }

    /// Create a new Range with the specified `from` and `to`
    pub fn new(from: DateTime<Utc>, to: Option<DateTime<Utc>>) -> Result<Range> {
        if to.is_none() {
            return Ok(Range { from, to });
        }

        return if from + Duration::seconds(1) <= to.unwrap() {
            Ok(Range { from, to })
        } else {
            bail!("Range is invalid: from cannot be greater that to");
        };
    }

    /// Create a new Range representing the day containing the given date/time
    pub fn day(day: &DateTime<Local>) -> Result<Range> {
        let morning = match Utc.from_local_datetime(&day.naive_utc().date().and_hms(0, 0, 0)) {
            LocalResult::Single(t) => t,
            _ => bail!("Cannot determine morning"),
        };
        let evening = match Utc.from_local_datetime(&day.naive_utc().date().and_hms(23, 59, 59)) {
            LocalResult::Single(t) => Some(t),
            _ => bail!("Cannot determine evening"),
        };

        Range::new(morning, evening)
    }

    /// Create a Range representing today
    pub fn today() -> Result<Range> {
        Self::day(&Local::today().and_hms(0, 0, 0))
    }

    /// Create a Range representing yesterday
    pub fn yesterday() -> Result<Range> {
        let day = Local::today() - Duration::days(1);
        Self::day(&day.and_hms(0, 0, 0))
    }

    /// Create a new Range representing the week containing the given date/time
    pub fn week(day: &DateTime<Local>) -> Result<Range> {
        let mut current = day.clone();
        while current.weekday() != Weekday::Mon {
            current = current - Duration::days(1);
        }

        let monday = match Utc.from_local_datetime(&current.naive_utc().date().and_hms(0, 0, 0)) {
            LocalResult::Single(t) => t,
            _ => bail!("Cannot determine morning"),
        };

        let sunday = match Utc.from_local_datetime(
            &(current + Duration::days(6))
                .naive_utc()
                .date()
                .and_hms(23, 59, 59),
        ) {
            LocalResult::Single(t) => Some(t),
            _ => bail!("Cannot determine morning"),
        };

        Range::new(monday, sunday)
    }

    /// Create a Range representing the current week
    pub fn current_week() -> Result<Range> {
        Self::week(&Local::today().and_hms(0, 0, 0))
    }

    /// Create a Range representing last week
    pub fn last_week() -> Result<Range> {
        let day = Local::today() - Duration::days(7);
        Self::week(&day.and_hms(0, 0, 0))
    }

    /// Create a new Range representing the month containing the given date/time
    pub fn month(day: &DateTime<Local>) -> Result<Range> {
        let mut current = day.clone();
        while current.day() != 1 {
            current = current - Duration::days(1);
        }

        let first = match Utc.from_local_datetime(&current.naive_utc().date().and_hms(0, 0, 0)) {
            LocalResult::Single(t) => t,
            _ => bail!("Cannot determine morning"),
        };

        current = current + Duration::days(26);
        while current.day() != 1 {
            current = current + Duration::days(1);
        }

        let last = match Utc.from_local_datetime(
            &(current - Duration::days(1))
                .naive_utc()
                .date()
                .and_hms(23, 59, 59),
        ) {
            LocalResult::Single(t) => Some(t),
            _ => bail!("Cannot determine morning"),
        };

        Range::new(first, last)
    }

    /// Create a Range representing the current month
    pub fn current_month() -> Result<Range> {
        Self::month(&Local::today().and_hms(0, 0, 0))
    }

    /// Create a Range representing the last month
    pub fn last_month() -> Result<Range> {
        let mut current = Local::today();
        let this_month = current.month();
        while current.month() != this_month - 1 {
            current = current - Duration::days(15);
        }
        Self::month(&current.and_hms(0, 0, 0))
    }

    /// Return true if the range is open. An open range is a Range that has no end set.
    pub fn is_open(&self) -> bool {
        self.to.is_none()
    }

    /// Return the intersection with another Range, if any.
    pub fn intersection(&self, other: &Range) -> Option<Range> {
        if self.to.is_none() && other.to.is_none() {
            let from = if self.from < other.from {
                other.from
            } else {
                self.from
            };
            return match Range::new(from, None) {
                Ok(r) => Some(r),
                Err(_) => None,
            };
        }

        let from_a = self.from;
        let to_a = self.to;

        let from_b = other.from;
        let to_b = other.to;

        if let Some(to) = to_a {
            if from_b > to {
                return None;
            }
        } else {
            let from = if self.from < other.from {
                other.from
            } else {
                self.from
            };
            return match Range::new(from, to_b) {
                Ok(r) => Some(r),
                Err(_) => None,
            };
        }

        if let Some(to) = to_b {
            if from_a > to {
                return None;
            }
        } else {
            let from = if self.from < other.from {
                other.from
            } else {
                self.from
            };
            return match Range::new(from, to_a) {
                Ok(r) => Some(r),
                Err(_) => None,
            };
        }

        if let Some(to) = to_b {
            if from_a > to {
                return None;
            }
        }

        let from = if from_a > from_b { from_a } else { from_b };
        let to = if to_a.unwrap() > to_b.unwrap() {
            to_b
        } else {
            to_a
        };

        match Range::new(from, to) {
            Ok(r) => Some(r),
            Err(_) => None,
        }
    }

    /// Return the duration of a Range
    pub fn duration(&self) -> Duration {
        let to = match self.to {
            Some(t) => t,
            None => Utc::now(),
        };

        to - self.from
    }

    /// Return the number of days in the Range
    pub fn days(&self) -> Vec<DateTime<Utc>> {
        let mut current = self.from;
        let end = self.to.unwrap_or(Utc::now());
        let mut days = vec![];
        while current <= end {
            days.push(current);
            current = current.add(Duration::days(1));
        }

        days
    }

    /// Split the Range at the given date/time in 2 new Range elements
    pub fn split_at(&self, date: DateTime<Utc>) -> Result<(Range, Range)> {
        let to = match self.to {
            Some(t) => t,
            None => Utc::now(),
        };

        let duration_from = date - self.from;
        let duration_to = to - date;

        Ok((
            Range::new(self.from, Some(self.from + duration_from))?,
            Range::new(to - duration_to, self.to)?,
        ))
    }

    /// Split the Range at middle in 2 new Range elements
    pub fn split(&self) -> Result<(Range, Range)> {
        let to = match self.to {
            Some(t) => t,
            None => Utc::now(),
        };

        let duration = (to - self.from) / 2;

        self.split_at(self.from + duration)
    }
}

impl FromStr for Range {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let range = match parse_range(s) {
            Ok((st, r)) => {
                if !st.is_empty() {
                    bail!("Cannot parse range {}", s)
                }
                r
            }
            Err(_) => bail!("Cannot parse range {}", s),
        };

        if let None = range.to {
            ensure!(
                Utc::now() - range.from >= Duration::seconds(1),
                "From must be less than \"now - 1s\""
            )
        }

        Ok(range)
    }
}

impl Display for Range {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let from = Local.from_utc_datetime(&self.from.naive_utc());
        if self.is_open() {
            write!(
                f,
                "{} - ... [{}]",
                from,
                Range::pretty_duration(&self.duration())
            )
        } else {
            let to = Local.from_utc_datetime(&self.to.unwrap().naive_utc());
            write!(
                f,
                "{} - {} [{}]",
                from,
                to,
                Range::pretty_duration(&self.duration())
            )
        }
    }
}

/// Represent a time entry in timewarrior. It stores the time Range, the tags and the id of the
/// entry.
#[derive(Clone)]
pub struct TimeEntry {
    range: Range,
    tags: Vec<String>,
    id: usize,
}

impl TimeEntry {
    /// Return the time Range of the entry. It can be open if the entry is currently being logged.
    pub fn range(&self) -> &Range {
        &self.range
    }

    /// Return a slice of the entry's tags.
    pub fn tags(&self) -> &[String] {
        &self.tags
    }

    /// Return the day of this entry. Note that this is the day of the start of the entry.
    pub fn day(&self) -> NaiveDate {
        self.range.from.naive_local().date()
    }

    /// Return the ID of the entry. IDs are not fixed for a specific entry. IDs are always counting
    /// up from one, starting at the most recent entry.
    pub fn id(&self) -> usize {
        self.id
    }
}

impl FromStr for TimeEntry {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match parse_entry(s) {
            Ok((_, e)) => Ok(e),
            Err(_) => bail!("Cannot parse \"{}\"", s),
        }
    }
}

impl Display for TimeEntry {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {:?}", self.range(), self.tags())
    }
}

/// Represent the work done, providing a list of time entries.
pub struct Work {
    entries: Vec<TimeEntry>,
}

impl Work {
    fn load_entries_from_file(file: &Path) -> Result<Vec<TimeEntry>> {
        let mut entries = vec![];
        let data = File::open(file)?;

        for line in io::BufReader::new(data).lines() {
            entries.push(line?.parse()?);
        }

        Ok(entries)
    }

    /// Load entries from the given timewarrior database at data_path.
    /// If Range is given, only the entries in that range are added to the Work.
    pub fn load_range(data_path: &Path, range: Option<Range>) -> Result<Work> {
        let mut entries = vec![];

        let file_re = Regex::new(r"^(?P<y>\d{4})-(?P<m>\d{2}).data$").unwrap();

        for file in read_dir(data_path)? {
            let file = file?;
            for _ in file_re.captures_iter(&file.file_name().to_string_lossy()) {
                entries.append(&mut Work::load_entries_from_file(&file.path())?);
            }
        }

        entries.sort_by(|a, b| {
            if a.range.from > b.range.from {
                Ordering::Less
            } else if a.range.from < b.range.from {
                Ordering::Greater
            } else {
                Ordering::Equal
            }
        });

        let mut i = 1;
        for e in &mut entries {
            e.id = i;
            i += 1;
        }

        return if let Some(r) = range {
            Ok(Work {
                entries: entries
                    .into_iter()
                    .filter(|e| e.range.intersection(&r).is_some())
                    .collect(),
            })
        } else {
            Ok(Work { entries })
        };
    }

    /// Same as `load_range` but loads all entries.
    pub fn load_all(data_path: &Path) -> Result<Work> {
        Work::load_range(data_path, None)
    }

    /// Return a slice of the entries
    pub fn entries(&self) -> &[TimeEntry] {
        &self.entries
    }

    /// Return the duration of all the entries.
    pub fn duration(&self) -> Duration {
        self.entries()
            .iter()
            .fold(Duration::zero(), |a, t| a + t.range.duration())
    }
}

impl Display for Work {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} entries loaded", self.entries.len())
    }
}

#[cfg(test)]
mod range_tests {
    use crate::data::Range;
    use chrono::{DateTime, Duration, NaiveDateTime, Utc};

    fn parse_date_time(date: &str) -> DateTime<Utc> {
        let d = NaiveDateTime::parse_from_str(date, "%Y%m%dT%H%M%SZ").unwrap();
        DateTime::<Utc>::from_utc(d, Utc)
    }

    #[test]
    fn test_range_parse() {
        // Correct range
        assert_eq!(
            "20220101T120000Z - 20220101T130000Z"
                .parse::<Range>()
                .unwrap(),
            Range {
                from: parse_date_time("20220101T120000Z"),
                to: Some(parse_date_time("20220101T130000Z"))
            }
        );

        // from > to
        assert!("20220101T130000Z - 20220101T120000Z"
            .parse::<Range>()
            .is_err());

        // to not specified
        assert_eq!(
            "20220101T120000Z".parse::<Range>().unwrap(),
            Range {
                from: parse_date_time("20220101T120000Z"),
                to: None
            }
        );

        // to not specified but from > now
        let now = Utc::now();
        let tomorrow = now + Duration::days(1);
        let range = tomorrow.format("%Y%m%dT%H%M%SZ").to_string();
        assert!(range.parse::<Range>().is_err(),);

        // Wrong format
        assert!("１日１月２０２２年".parse::<Range>().is_err())
    }

    #[test]
    fn test_range_split() {
        let input1: Range = "20220101T120000Z - 20220101T130000Z".parse().unwrap();
        let output1: Range = "20220101T120000Z - 20220101T123000Z".parse().unwrap();
        let output2: Range = "20220101T123000Z - 20220101T130000Z".parse().unwrap();

        assert_eq!(input1.split().unwrap(), (output1, output2));

        // Check that 1s duration is not split
        let input1: Range = "20220101T120000Z - 20220101T120001Z".parse().unwrap();

        assert!(input1.split().is_err());
    }

    #[test]
    fn test_range_split_at() {
        let input1: Range = "20220101T120000Z - 20220101T130000Z".parse().unwrap();
        let output1: Range = "20220101T120000Z - 20220101T123000Z".parse().unwrap();
        let output2: Range = "20220101T123000Z - 20220101T130000Z".parse().unwrap();

        // Correct split
        assert_eq!(
            input1
                .split_at(parse_date_time("20220101T123000Z"))
                .unwrap(),
            (output1, output2)
        );

        // Split gives range < 1s
        assert!(input1
            .split_at(parse_date_time("20220101T120000Z"))
            .is_err());
        assert!(input1
            .split_at(parse_date_time("20220101T130000Z"))
            .is_err());

        // Split out of bounds
        assert!(input1
            .split_at(parse_date_time("20220101T110000Z"))
            .is_err());
        assert!(input1
            .split_at(parse_date_time("20220101T150000Z"))
            .is_err());
    }

    #[test]
    fn test_range_intersect() {
        // Check working intersection
        let input1: Range = "20220101T120000Z - 20220101T124500Z".parse().unwrap();
        let input2: Range = "20220101T123000Z - 20220101T130000Z".parse().unwrap();
        let output2: Range = "20220101T123000Z - 20220101T124500Z".parse().unwrap();

        assert_eq!(input1.intersection(&input2), Some(output2));

        // Check close to intersection
        let input1: Range = "20220101T120000Z - 20220101T123000Z".parse().unwrap();
        let input2: Range = "20220101T123000Z - 20220101T130000Z".parse().unwrap();

        assert!(input1.intersection(&input2).is_none());

        // Check no intersection
        let input1: Range = "20220101T120000Z - 20220101T121500Z".parse().unwrap();
        let input2: Range = "20220101T124500Z - 20220101T130000Z".parse().unwrap();

        assert!(input1.intersection(&input2).is_none());

        // Check intersection with 1 opened range
        let input1: Range = "20220711T120000Z".parse().unwrap();
        let input2: Range = "20220711T000000Z - 20220712T000000Z".parse().unwrap();
        let output: Range = "20220711T120000Z - 20220712T000000Z".parse().unwrap();

        assert_eq!(input1.intersection(&input2).unwrap(), output);
        assert_eq!(input2.intersection(&input1).unwrap(), output);

        // Check intersection of non-crossing range with 1 opened range
        let input1: Range = "20220718T095832Z".parse().unwrap();
        let input2: Range = "20220711T000000Z - 20220717T235959Z".parse().unwrap();

        assert!(input1.intersection(&input2).is_none());
        assert!(input2.intersection(&input1).is_none());

        // Check intersection of 2 opened ranges
        let input1: Range = "20220718T104206Z".parse().unwrap();
        let input2: Range = "20220717T000000Z".parse().unwrap();

        assert_eq!(input1.intersection(&input2).unwrap(), input1);
        assert_eq!(input2.intersection(&input1).unwrap(), input1);
    }

    #[test]
    fn test_range_duration() {
        let input1: Range = "20220101T120000Z - 20220101T124500Z".parse().unwrap();

        assert_eq!(input1.duration(), Duration::minutes(45));
    }
}

#[cfg(test)]
mod timeentry_tests {
    use crate::data::TimeEntry;
    use chrono::{DateTime, Duration, NaiveDateTime, Utc};

    fn parse_date_time(date: &str) -> DateTime<Utc> {
        let d = NaiveDateTime::parse_from_str(date, "%Y%m%dT%H%M%SZ").unwrap();
        DateTime::<Utc>::from_utc(d, Utc)
    }

    #[test]
    fn test_timeentry_parse() {
        let input1: TimeEntry = "inc 20220101T120000Z - 20220101T124500Z # tag1 \"tag 2\" tag3"
            .parse()
            .unwrap();

        assert_eq!(input1.range().duration(), Duration::minutes(45));
        assert_eq!(input1.tags(), vec!["tag1", "tag 2", "tag3"]);
        assert!(!input1.range().is_open());

        let input1: TimeEntry = "inc 20220101T120000Z # tag1 \"tag 2  \" \" t a g 3 \""
            .parse()
            .unwrap();

        assert_eq!(input1.range().from, parse_date_time("20220101T120000Z"));
        assert_eq!(input1.tags(), vec!["tag1", "tag 2  ", " t a g 3 "]);
        assert!(input1.range().is_open());
    }
}
