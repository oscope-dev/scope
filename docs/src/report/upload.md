# Report

The `scope report` command is used to generate a bug report. The bug report is defined in a `ScopeReportDefinition`.

When reporting a bug, scope will auto redact well known keys and patterns to reduce sharing private information.

## `ScopeReportDefinition`

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

- `.spec.additionalData` defines additional data that needs to be pulled from the system when reporting a bug. `additionalData` is a map of `string:string`, the value is a command that should be run. When a report is built, the commands will be run and automatically included in the report.
- `.spec.template` is a Jinja2 style template, to be included. The text should be in Markdown format. Scope injects `command` as the command that was run.

## `ScopeReportLocation`

Define where to upload the bug report to. Currently, only support `GitHubIssues` and `rustyPaste`.

### GitHub Issues

When reporting to GitHub Issues, the env-var `GH_TOKEN` must be set to get the API token.

Example in `report.yaml`

```yaml
---
apiVersion: scope.github.com/v1alpha
kind: ScopeReportLocation
metadata:
  name: github
spec:
  destination:
    githubIssue:
      owner: ethankhall
      repo: dummy-repo
```

### RustyPaste

[RustyPaste](https://github.com/orhun/rustypaste) is a pastebin style application. This may be a better choice when you can't require GitHub API token, or if there is a risk of sensitive data that shouldn't be in GitHub.

Example in `report.yaml`

```yaml
---
apiVersion: scope.github.com/v1alpha
kind: ScopeReportLocation
metadata:
  name: report
spec:
  destination:
    rustyPaste:
      url: http://localhost:8000
```

