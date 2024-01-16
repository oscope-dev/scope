# Scope

Scope allows teams to define development config for engineers. Scope is a tool for engineers, to make them more productive.

The config falls into three major categories:
- [Local setup](./doctor/index.md)
- [Error debugging locally](./errors/known-error.md)
- [Generate great bug reports](./report/upload.md)

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

## Commands

`scope` has 2 built-in commands that most engineers will use:
- `doctor` - Run checks that will "checkup" your machine
- `report` - Generate a bug report based from a command

Beyond the built-in command, scope will also run any binary prefixed with `scope-`.

Additionally, there is `list` that will output all the found configuration.

Here is an example output of `scope list` run from the `examples/` directory.

```text
17:47:57  INFO More detailed logs at /tmp/scope/scope-root-20240112-SxCM.log
17:47:57  INFO Doctor Checks
17:47:57  INFO         Name                                Description                                           Path
17:47:57  INFO ------------------------------------------------------------------------------------------------------------------------
17:47:57  INFO     path-exists                Check your shell for basic functionality             .scope/doctor-check-path-exists.yaml
17:47:57  INFO path-exists-fix-in-scope-dir           Check your shell for basic functionality            .scope/doctor-check-fix-in-scope.yaml
17:47:57  INFO Doctor Setup
17:47:57  INFO         Name                                Description                                           Path
17:47:57  INFO ------------------------------------------------------------------------------------------------------------------------
17:47:57  INFO        setup                          You need to run bin/setup                           .scope/doctor-setup.yaml
17:47:57  INFO
17:47:57  INFO Known Errors
17:47:57  INFO         Name                                Description                                           Path
17:47:57  INFO ------------------------------------------------------------------------------------------------------------------------
17:47:57  INFO     error-exists                Check if the word error is in the logs                    .scope/known-error.yaml
17:47:57  INFO
17:47:57  INFO Commands
17:47:57  INFO         Name                                Description
17:47:57  INFO --------------------------------------------------------------------------------
17:47:57  INFO         bar                 External sub-command, run `scope bar` for help
17:47:57  INFO        doctor                Run checks that will "checkup" your machine
17:47:57  INFO         foo                 External sub-command, run `scope foo` for help
17:47:57  INFO         list             List the found config files, and resources detected
17:47:57  INFO        report          Generate a bug report based from a command that was ran
```

Under the `Commands` section, notice two additional commands:
- `bar` which is located in `.scope/bin/scope-bar`
- `foo` which is a binary on the `PATH`

Scope automatically adds `.scope/bin` to the path when searching. Allowing teams to add commands to scope in large repos. For example, in a mono-repo with multiple services, you may want to add a `deploy` command. The `deploy` command would come from the working dir.

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