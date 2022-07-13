// This will contain all functions that edit the time entries (start, stop, split, remove, ...)

use crate::data::TimeEntry;
use anyhow::{bail, Result};

// Stop the current tracking, start a new tracking with the new tags and return the time entry
pub fn start(tags: &str) -> Result<TimeEntry> {
    bail!("Not implemented yet");
}
