# Doctor Check

Check instructions are used to verity the environment has dependencies working, usually things like a database or dependent services.

Looking at `doctor-check-path-exists.yaml` in the example folder.

```yaml
apiVersion: scope.github.com/v1alpha
kind: ScopeDoctorCheck
metadata:
  name: path-exists
spec:
  check:
    target: scripts/does-path-env-exist.sh
  fix:
    target: ../bin/truey
  description: Check your shell for basic functionality
  help: Your shell does not have a path env. Reload your shell.
```

The kind is `ScopeDoctorCheck`, letting scope know that this is a Check instruction.

## Schema

- `.spec.check.target` is a script to run, used to check if the system is working. If the script exits 0, that indicates success. Otherwise, that something is wrong. The scripts are relative to the folder containing spec file. If this file was at `$HOME/workspace/example/.scope/check.yaml` the command to run would be  `$HOME/workspace/example/.scope/scripts/does-path-env-exist.sh` 

- `.spec.fix.target` is an optional command to run when the check fails. If provided, the fix will automatically run.

- `.spec.description` is a useful description of the setup, used when listing what's available.

- `.spec.help` text to provide to the user when the check fails, and if the fix fails.

- `.spec.order` a number, defaulting to 100, that will change the order the step is run in. The lower the number, the earlier the step will be run.
