# Doctor Setup 

Setup instructions are used to install dependencies, run db migrations, and other changes based on files present in the repo. These instructions will most often be run after a `git pull` or other similar step.

Looking at `doctor-setup.yaml` in the examples repo

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

## Schema

- `.spec.cache.paths` is an array of `globstar` files that will be used to check for changes. These paths are relative to the "project dir". If this file was at `$HOME/workspace/example/.scope/doctor-setup.yaml` the project dir would be `$HOME/workspace/example`. For this example, the search glob would be `$HOME/workspace/example/**/requirements.txt`.

- `.spec.setup.exec` is an array of commands to run when the cache is "busted". The scripts are relative to the folder containing spec file. If this file was at `$HOME/workspace/example/.scope/doctor-setup.yaml` the command to run would be  `$HOME/workspace/example/.scope/../bin/pip-install.sh`

- `.spec.description` is a useful description of the setup, used when listing what's available.

- `.spec.order` a number, defaulting to 100, that will change the order the step is run in. The lower the number, the earlier the step will be run.