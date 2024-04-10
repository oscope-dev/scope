---
sidebar_position: 1
---

# Scope

Scope allows teams to define development config for engineers. Scope is a tool for engineers, to make them more productive.

The config falls into three major categories:
- [Local setup](./commands/doctor.md)
- [Error debugging locally](models/ScopeKnownError.mdx)
- [Generate great bug reports](./commands/report.md)

## Config

Configuration for `scope` is done via Kubernetes-like yaml files that live inside the repo. By default, scope will search up for the config directory `.scope` and ready `*.yaml` and `*.yml` files.

That means `.scope/foo.yaml` will be parsed as a config file, but `scope/foo.yaml` will not (notice the missing `.` before `scope`).

A config file looks like

```yaml
apiVersion: scope.github.com/v1alpha
kind: <kind>
metadata:
  name: a useful name
spec:
  ...
```

Currently, the only supported `apiVersion` is `scope.github.com/v1alpha`. Using `apiVersion` allows scope to evolve the config file and keep older versions of the config compatible.

Unlike Kubernetes, the `name` field can be any string, without any DNS related constraints.

Additionally, scope will look for shared config in the directory `../etc/scope` relative to where the executable is located.

To exclude files from being loaded, put a [gitignore style](https://git-scm.com/docs/gitignore) file named `.ignore` into a config directory.

## CLI Options
All commands support the following options

```text
Usage: scope [OPTIONS] <COMMAND>

Options:
  -d, --debug...                     A level of verbosity, and can be used multiple times
  -w, --warn                         Enable warn logging
  -e, --error                        Disable everything but error logging
      --extra-config <EXTRA_CONFIG>  Add a paths to search for configuration. By default, `scope` will search up for `.scope` directories and attempt to load `.yml` and `.yaml` files for config. If the config directory is somewhere else, specifying this option will _add_ the paths/files to the loaded config [env: SCOPE_CONFIG_DIR=]
      --disable-default-config       When set, default config files will not be loaded and only specified config will be loaded [env: SCOPE_DISABLE_DEFAULT_CONFIG=]
  -C, --working-dir <WORKING_DIR>    Override the working directory
      --run-id <RUN_ID>              When outputting logs, or other files, the run-id is the unique value that will define where these go. In the case that the run-id is re-used, the old values will be overwritten [env: SCOPE_RUN_ID=]
  -h, --help                         Print help (see more with '--help')
  -V, --version                      Print version
```

Normally, you will not need to set any of these files.

## Environment Variables

Scope will load environment variables in this order of presedence:

1. `../etc/scope.env` relative to where the executable is located
1. `.env` in the current working directory
