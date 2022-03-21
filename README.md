# Chain Monitor

Simple web-UI based tool to monitor chain heights of various blockchains
as reported by different sources.

# Status

Still early, but basically working. Check live demo on https://chainmonitor.info

## Goals

Done:

* support both single-user (local) and web-service deployment cases,
* keep relatively simple (tiny Rust backend + single-page + light plain-JS UI),
* optional sound notifications

Near:

* musl-based static binary releases,
* indicate lack of progress (source height not increasing),
* bubble up source update errors (icons + hint on hover?),
* expose Prometheus metrics for use inside cloud infra,
* opportunistic chain fork detection (mismatched hashes),
* add more data sources,
* adaptive update frequency,

Far:

* stabilize websocket (and any other) APIs,


### Contributing

As long as you want to keep the spirit, I'm very happy to accept contributions.
Forking very much welcome.

License: MPL-2.0 OR MIT OR Apache-2.0
