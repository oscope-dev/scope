---
sidebar_position: 11
---

# ScopeDoctorSetup

**Deprecated**, use [ScopeDoctorGroup](./ScopeDoctorGroup.md) instead.
Scope will translate this file to a Group when running internally.

```yaml
apiVersion: scope.github.com/v1alpha
kind: ScopeDoctorSetup
metadata:
  name: setup
spec:
  # order: 100 # default value
  cache:
    paths:
      - '**/requirements.txt'
  setup:
    exec:
      - ../bin/pip-install.sh
  description: You need to run bin/setup
```

The kind is `ScopeDoctorSetup`, letting scope know that this is a Setup instruction.

## Exit Codes

Depending on the exit code, different effects will happen

| Exit Code   | Setup                                              |
|-------------|----------------------------------------------------|
| `0`         | Fix was successful                                 |
| `1 - 99`    | Fix ran, but failed, other steps should still run. |
| `100+`      | Fix ran, failed, and execution should stop         |

## Schema

- `.spec.cache.paths` is an array of `globstar` files that will be used to check for changes. These paths are relative to the "project dir". If this file was at `$HOME/workspace/example/.scope/doctor-setup.yaml` the project dir would be `$HOME/workspace/example`. For this example, the search glob would be `$HOME/workspace/example/**/requirements.txt`.

- `.spec.setup.exec` is an array of commands to run when the cache is "busted". The scripts are relative to the folder containing spec file. If this file was at `$HOME/workspace/example/.scope/doctor-setup.yaml` the command to run would be  `$HOME/workspace/example/.scope/../bin/pip-install.sh`

- `.spec.description` is a useful description of the setup, used when listing what's available.

- `.spec.order` a number, defaulting to 100, that will change the order the step is run in. The lower the number, the earlier the step will be run.
