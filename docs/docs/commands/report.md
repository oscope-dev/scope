---
sidebar_position: 2
---

# Report

`scope report` is used to generate a bug report.

To build a bug report, two different files are used:

- [ScopeReportDefinition](../models/ScopeReportDefinition.md) - is used to define what to include in the bug report. There can only be one report definition at one time.
- [ScopeReportLocation](../models/ScopeReportLocation.md) - defines where to upload reports to.

When reporting a bug, scope will auto redact well known keys and patterns to reduce sharing private information.

The output from the command will be captured and uploaded.

## Special Thanks

We took our redaction string from [sirwart/ripsecrets](https://github.com/sirwart/ripsecrets).