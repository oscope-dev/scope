# Scope

`scope` is a tool that allows developer experience teams to provide tooling for engineers.

There are several tools engineers will use directly:

- `scope doctor` runs a set of user-defined scripts to help configure, debug, and fix an engineers environment.

- `scope analyze` checks for instances of known errors in logs or command output, and provides help to resolve the issue.

- `scope report` is used to report local execution error.
  This is primarily used to generate a bug report, and upload it somewhere so that the responders have all the required into to respond.

For full documentation, please visit [our website](https://oscope-dev.github.io/scope/).

## Install

**Homebrew**

```shell
brew install oscope-dev/homebrew-formulas/dev-scope
```

**Install Script**

Review the [latest release](https://github.com/oscope-dev/scope/releases/latest) to find the install command.

**Cargo**

```shell
cargo install dev-scope
```

**Manually**

To install manually, download the correct archive for your platform from [github release](https://github.com/oscope-dev/scope/releases/latest), extract it, and place the binaries on the `PATH`.

## Commands

### `scope doctor`

Environments are hard to maintain and can fall out of sync with exceptions quickly.
`scope doctor` is a way to codify what a working environment means and tell the user how to fix it.
The aim is to reduce the need for engineers to ask others to fix their environment and distribute what "working" means to everyone.

### `scope analyze`

When a command fails and produces error output, it can be complex and hard to understand, especially for new engineers.

`scope analyze` will automatically check the command output or log file for known issues and provide help on how to resolve the error.

### `scope report`

Sometimes you need to report an error to others.
Often the responding team wants the output that failed, and some other useful debugging information.

By using `scope report some-command.sh`, scope will capture all the output with timestamps and then generate a "report" that can be uploaded to multiple destinations.

> [!IMPORTANT]
> `scope` will redact anything it finds as "sensitive".
> This allows you to fetch env-vars, and no leak GH API tokens for example.

### `scope-intercept`

`scope-intercept` is a [shebang](https://en.wikipedia.org/wiki/Shebang_(Unix)) replacement for `env`.
Behind the scene `env -S` is run to execute the script, however `scope-intercept` will watch all the output and then check for KnownErrors.
This allows the engineer to see, in real time, suggestions for fixing errors.
It also allows the engineer to upload a bug report immediately.

> [!IMPORTANT]
> `scope` will redact anything it finds as "sensitive".
> This allows you to fetch env-vars, and no leak GH API tokens for example.

## Special Thanks

We took our redaction string from [sirwart/ripsecrets](https://github.com/sirwart/ripsecrets).

