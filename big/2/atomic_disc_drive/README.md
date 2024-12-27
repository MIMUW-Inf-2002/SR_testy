## About

This is a small crate that should just execute
`run_register_process` in a tokio runtime with 3 worker threads
based on a config file.

## Usage
Before anything, you need to add `#[derive(Debug)]` to the `Configuration` struct
in `domain.rs`. (or you can delete logging from `main.rs` here if you don't want logs) 
```
cargo run <path_to_config> <self_rank> <storage_dir>
```
So if you want to run 3 processes, you can run them in separate
terminal windows by executing this command for each process.

The config file should contain
```
<system_hmac>
<client_hmac>
<num_of_sectors>
<host>
<port>
[<host>
<port>]
```
Where there should be as many `<host>` and `<port>` pairs as 
the number of processes in the system.

Example config file can be found in this directory.
## Remark
Currently, here we pass hmac as ASCII characters, while in the
[assignment instructions](https://www.mimuw.edu.pl/~iwanicki/courses/ds/2024/labs/LA2/linux_driver.html),
we provide the hmac in lower hex form in the `./insmod_example.sh` file.

Keep that in mind, that means that, for example, if you provide a hmac 
```
bbb
``` 
in the config file here,
you should input 
```
626262
```

To `insmod`. (of course hmac needs to be longer, this is just an example)
