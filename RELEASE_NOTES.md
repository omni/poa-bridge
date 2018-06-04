# 0.2.1

This release contains a number of bugfixes and a change in handling gas price.
It is no longer set statically but rather dynamically using an external oracle
(see [config example](examples/config.toml))

# 0.2.0

This release, most notably, fixes a condition in which not all logs might be
retrieved from an RPC endpoint, resulting in the bridge not being able to
see all relevant events.

It also improves the performance by introducing concurrent transaction batching.

On the operations side, it'll now print the context of an occurring error
before exiting to help investigating that error.

# 0.1.0

Initial release
