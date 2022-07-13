// This file will contain function that are used to format the data in a certain way
// functions like summ, day, week, month, tags, raw

use anyhow::Result;
use home::home_dir;

use crate::data::{Range, Work};

pub fn raw(range: Option<Range>) -> Result<Work> {
    let mut data_path = home_dir().unwrap();
    data_path.push(".timewarrior");
    data_path.push("data");

    Work::load_range(&data_path, range)
}
