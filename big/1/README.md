## tokio_unstable
Some tests use some features from tokio_unstable. For example to monitor for unhandled panics, which do not fail the tests by default.

In order to enable tokio_unstable, add `.cargo` directory from this repo to `public-tests` dir from the assignment. It should work then, but your IDE may throw some warnings which can be fixed by adding the following to the end of `Cargo.toml`

```
[lints.rust]
unexpected_cfgs = { level = "warn", check-cfg = ['cfg(tokio_unstable)'] }
```
