---
sidebar_position: 3
---

# ScopeKnownError

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

## Schema

- `.spec.pattern` is a Regex that will be run over stdout and stderr to search for a known error.
- `.spec.help` is the description given when the pattern matches
