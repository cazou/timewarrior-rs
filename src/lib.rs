//! # timewarrior-rs
//!
//! `timewarrior-rs` allows reading data logged by the `timew` utility. The current implementation
//! only allows retrieving the raw data in a `Vec<TimeEntry>`: A list of `TimeEntry` elements.
//!
//! Future improvement will allow formatting the data to be processed and shown like the different
//! `timew` utility commands.
//!
//! Even future work will allow adding entries in the database.
//! Usage example:
//!
//!     use timewarrior_rs::{ data, formatter };
//!     let range = data::Range::today().unwrap();
//!     let work = formatter::raw(Some(range)).unwrap();
//!     println!("{}", data::Range::pretty_duration(&work.duration()));
//!     for entry in work.entries() {
//!         println!("{entry}");
//!     }

/// Represent data as fetched from the database
pub mod data;

/// Format data depending on what needs to be displayed
pub mod formatter;

pub mod config;
pub mod editor;

