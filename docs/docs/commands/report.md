---
sidebar_position: 2
---

# Report

`scope report` is used to generate a bug report.

To build a bug report, one object is needed the [ScopeReportLocation](../models/ScopeReportLocation.mdx).

A [ScopeReportLocation](../models/ScopeReportLocation.mdx) defines where to upload the artifact to, what templates to use, and what additional data is needed when uploading the report.

When reporting a bug, scope will auto redact well known keys and patterns to reduce sharing private information.

The output from the command will be captured and uploaded.

## Special Thanks

We took our redaction string from [sirwart/ripsecrets](https://github.com/sirwart/ripsecrets).