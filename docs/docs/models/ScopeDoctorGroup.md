---
sidebar_position: 1
---

# ScopeDoctorGroup

A "group" is a set of operations that should be preformed as part of solving a problem.
For example, there may be a group to install and configure Node, or other language.

A Group is defined by one or more "actions".
Taking a look at an example

```yaml
apiVersion: scope.github.com/v1alpha
kind: ScopeDoctorGroup
metadata:
  name: node
spec:
  description: Check node is ready.
  actions:
    - description: Node Version
      check:
        paths:
          - '.npmrc'
        commands:
          - ./scripts/check-node-version.sh
      fix:
        commands:
          - ./scripts/fix-node-version.sh
    - description: Install packages
      check:
        paths:
          - 'package.json'
          - '**/package.json'
          - yarn.lock
          - '**/yarn.lock'
      fix:
        commands:
          - yarn install
```

In the example above, there are two actions, the first ensures that node is operational.

## Actions

An action is a discrete step, they run in order defined.
The action should be atomic, and target a single resolution.
If an action fails, the following actions will still run, unless `fix.commands` or `check.commands` exit with an error code `>=100`.

Notice an action can provide both `paths` and `commands`, if either of them indicate that the fix should run, it will run.
In the event there are no defined check, the fix will _always_ run.

## Fix

When the checks determine that something isn't correct, a fix is the way to automate the resolution.
When provided, `scope` will run them in order, if a command fails, the next command will continue to run unless the script exists with `>=100`

## Commands

A command can either be relative, or use the PATH.
To target a script relative to the group it must start with `.`, and giving a relative path to the group file.

## Exit Codes

Depending on the exit code, different effects will happen

| Exit Code   | Check Effect                                                       | Fix Effect                                 |
|-------------|--------------------------------------------------------------------|--------------------------------------------|
| `0`         | No work needed                                                     | Fix was successful                         |
| `1 - 99`    | Work required                                                      | Fix ran, but failed                        |
| `100+`      | Work is required, but fix should not run. Do not run other checks. | Fix ran, failed, and execution should stop |

## Schema

- `.spec.action` a series of steps to check and fix for the group.
- `.spec.action[].check.paths` A list of globs to check for a change.
- `.spec.action[].check.commands` A list of commands to execute, using the exit code to determine if changes are needed.
- `.spec.action[].fix.commands` A list of commands to execute, attempting to resolve issues detected by the action.
- `.spec.description` is a useful description of the setup, used when listing what's available.
