# 0.3.0

In this release, dynamic gas pricing has been added
(#85), allowing to fetch gas price from an external
oracle.

This release addresses some major performance issues
with how RPC requests were coordinated. With this release,
RPC requests are finally properly parallelized, leading
to much better overall performance (#94)

It also addresses some potential loss of database updates
under certain conditions (#95)

Along with this, some old configuration options were
deprecated.

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
