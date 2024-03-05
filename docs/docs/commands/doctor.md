---
sidebar_position: 1
---

# Doctor

Doctor is used to fix a local environment. To fix a machine, you'll need to provide either [ScopeDoctorCheck](../models/ScopeDoctorCheck.md) or [ScopeDoctorSetup](../models/ScopeDoctorSetup.md) files. Multiple are supported and recommended.

**Help Text**

```text
Run checks that will "checkup" your machine

Usage: scope doctor [OPTIONS] <COMMAND>

Commands:
  run   Run checks against your machine, generating support output
  list  List all doctor config, giving you the ability to know what is possible
  help  Print this message or the help of the given subcommand(s)
```

## `run`

`scope doctor run` is used to execute all the doctor steps. All checks will be run, if you want to only run specific checks, the `--only` flag with the name of the check to run. This option can be provided multiple times.

By default, any provided fix's will be run. If you don't want to run fixes add `--fix=false` to disable fixing issues.

When using a [ScopeDoctorSetup](../models/ScopeDoctorSetup.md), the checksum of files are stored on disk. If you need to disable caching, add `--no-cache`.

```text
Run checks against your machine, generating support output

Usage: scope doctor run [OPTIONS]

Options:
  -o, --only <ONLY>                  When set, only the checks listed will run
  -f, --fix <FIX>                    When set, if a fix is specified it will also run [default: true] [possible values: true, false]
  -n, --no-cache                     When set cache will be disabled, forcing all file based checks to run
(excluded default args)
```

## `list`

Will print out all doctor checks available, in the order `run` will execute.

```text
 INFO Available checks that will run
 INFO   Name                                           Description                                                 Path
 INFO - ScopeDoctorGroup/setup                         You need to run bin/setup                                   .scope/doctor-group-setup.yaml
 INFO - ScopeDoctorGroup/path-exists-fix-in-scope-dir  Check your shell for basic functionality                    .scope/doctor-group-in-scope-dir.yaml
 INFO - ScopeDoctorGroup/path-exists                   Check your shell for basic functionality                    .scope/doctor-group-path-exists.yaml
 INFO - ScopeDoctorGroup/group-1                       Check your shell for basic functionality                    .scope/doctor-group-1.yaml
```