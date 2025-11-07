# Running tests

To run these tests put the `kwasow.rs` (or any other test file) in the
`public-tests/tests` directory. Then follow the next section to enable
proper options for the tokio library.

To run the tests, just execute `cargo test` inside the `public-tests`
directory.

## tokio\_unstable

Some tests use tokio_unstable features to to monitor for unhandled panics
in tokio tasks, which do not cause a test failure by default - if your
solution panics while exiting from some tokio task, it will not be detected
by the tests, unless the below options are set.

In order to enable tokio_unstable, add a `.cargo` directory from this repo
(if not already present) to the `public-tests` directory. To make sure that
it is working, verify that when you run `cargo test` your output is similar
to this:

```
     Running tests/kwasow.rs (target/debug/deps/kwasow-3ddf86643e50d8d6)

running 10 tests
test kwasow_idle_test ... ok
test kwasow_collatz_2_test ... ok
test kwasow_collatz_1_test ... ok
test kwasow_collatz_3_test ... ok
test kwasow_shutdown_test_3 ... ok
test kwasow_collatz_shutdown_test ... ok
test kwasow_shutdown_test_1 ... ok
test kwasow_shutdown_test_2 ... ok
test kwasow_timer_drift_test ... ok
test kwasow_timer_efficiency_test ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.02s
```

Pay special attention to the thirs line, where the number of tests is displayed.

