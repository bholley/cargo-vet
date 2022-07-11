# Specifying Policies

By default, `cargo vet` checks all transitive dependencies of all top-level
crates against the following criteria on all-platforms:
* For regular dependencies: `safe-to-deploy`
* For dev-dependencies and build-dependencies: `safe-to-run`

In some situations, you may be able to reduce your workload by encoding your
requirements more precisely, such as:

* **Using different criteria for different top-level crates.** Your workspace
  might contain both a production product and an internal tool, which might
require different levels of audits for their dependencies.

If the default behavior works for you, there's no need to specify anything. If
you wish to encode policies such as the above, you can do so in
[config.toml](config.md#the-policy-table).
