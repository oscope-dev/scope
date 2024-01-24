---
sidebar_position: 4
---

# Models

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