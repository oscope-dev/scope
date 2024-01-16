# Known Errors

A Known Error, provides a way for repo owners to describe errors that others may run into, and how to fix them.

A known error is only used when using the `intercept` binary.

Looking at `known-error.yaml` in the examples directory.

```yaml
apiVersion: scope.github.com/v1alpha
kind: ScopeKnownError
metadata:
  name: error-exists
spec:
  description: Check if the word error is in the logs
  pattern: error
  help: The command had an error, try reading the logs around there to find out what happened.
```

- `.spec.pattern` is a Regex that will be run over stdout and stderr to search for a known error.
- `.spec.help` is the description given when the pattern matches
