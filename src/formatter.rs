// This file will contain function that are used to format the data in a certain way
// functions like summ, day, week, month, tags, raw

use anyhow::Result;
use home::home_dir;

use crate::data::{Range, Work};

/// Get the raw data for the given time Range.
///
/// If range is not specified, the whole database is retrieved.
/// The data is always retrieved from the `${HOME}/.timewarrior/data` file.
pub fn raw(range: Option<Range>) -> Result<Work> {
    let mut data_path = home_dir().unwrap();
    data_path.push(".timewarrior");
    data_path.push("data");

    Work::load_range(&data_path, range)
}
