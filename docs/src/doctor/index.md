# Scope Doctor

Scope supports two types of management instructions. `Setup` instructions are based on file contents, and `Check` instructions are based on scripts.

These instructions are run under the `doctor` subcommand. Both `Check` and `Setup` are run when `doctor` is run.

Both `Check` and `Setup` support `.spec.order` option, defaulting to 100. This allows you to specify that a command doctor step should be run earlier or later in the doctor process.

The `doctor` command will also run all the parent doctor steps. So in a large repo, multiple different levels can run.

To override a `doctor` step, create a new step, with the same `kind` and the same name. If they do not share the same `kind`, they will not be overwritten.