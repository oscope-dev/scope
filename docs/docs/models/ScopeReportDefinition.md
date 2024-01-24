---
sidebar_position: 4
---

# ScopeReportDefinition

There can only be one `ScopeReportDefinition` at any time. If there are multiple when searching, the "closest" is used.

Looking at `report.yaml` in the example directory.

```yaml
apiVersion: scope.github.com/v1alpha
kind: ScopeReportDefinition
metadata:
  name: template
spec:
  additionalData:
    username: id -u
    ruby: which ruby
    node: which node
    nodeVersion: node -v
  template: |
    # There was an error!
    
    When running `{{ command }}` scope ran into an error
```

## Schema

- `.spec.additionalData` defines additional data that needs to be pulled from the system when reporting a bug. `additionalData` is a map of `string:string`, the value is a command that should be run. When a report is built, the commands will be run and automatically included in the report.
- `.spec.template` is a Jinja2 style template, to be included. The text should be in Markdown format. Scope injects `command` as the command that was run.