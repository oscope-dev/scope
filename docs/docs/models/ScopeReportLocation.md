---
sidebar_position: 5
---

# ScopeReportLocation

Define where to upload the bug report to. Currently, only support `GitHubIssues` and `rustyPaste`.

### GitHub Issues

When reporting to GitHub Issues, the env-var `GH_TOKEN` must be set to get the API token.

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
