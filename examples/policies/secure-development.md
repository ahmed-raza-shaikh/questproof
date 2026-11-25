---
title: Secure Development and Vulnerability Management Policy
owner: Engineering
last_reviewed: 2026-07-01
---

# Secure Development and Vulnerability Management Policy

## Code review

All code changes require peer review and approval by at least one other engineer before merging to the main branch.

## Change management

Production changes are deployed only through the CI/CD pipeline. Emergency changes are documented and retroactively approved within 2 business days.

## Static analysis and dependency scanning

Static application security testing and software composition analysis run on every pull request. Builds with critical findings are blocked.

## Penetration testing

An independent third party performs a penetration test of the production application at least annually. Executive summaries are available to customers under NDA.

## Vulnerability remediation

Vulnerabilities are remediated according to severity: critical within 7 days, high within 30 days, medium within 90 days.

## Environment separation

Production and development environments are logically separated, and production data is not used in development or testing environments.
