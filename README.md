`timewarrior-rs` is a library providing access to the timewarrior time tracking utility.
It currently only provides access to the data parsed from the local database.

Example
-------

```
use timewarrior_rs::formatter;

fn main() -> Result<(), String> {
    let range = Range::today().unwrap();

    println!("Loading TimeWarrior for {}... ", range);
    let work = formatter::raw(Some(range)).unwrap();

    for entry in work.entries() {
        println!("{entry}");
    }
    
    Ok(())
}
```

Will show the work of today

Future work
-----------

The next steps include:
 - Providing structs to easily show the output of the different timewarrior commands.
 - Add database editing through start/stop/modify/...
