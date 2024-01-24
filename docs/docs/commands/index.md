---
sidebar_position: 2
---

# Commands

`scope` has several built-in commands that most engineers will use:
- [`doctor`](doctor.md) - Run checks that will "checkup" your machine
- [`report`](report.md) - Generate a bug report based from a command
- [`analyze`](analyze/index.md) - Analyze configuration and print validation messages

Beyond the built-in command, scope will also run any binary prefixed with `scope-`.

Additionally, there is `list` that will output all the found configuration.

Here is an example output of `scope list` run from the `examples/` directory.

```text
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