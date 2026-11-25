---
title: Logging and Monitoring Standard
owner: Security Operations
last_reviewed: 2026-05-12
---

# Logging and Monitoring Standard

## Audit logging

Security-relevant events, including authentication, privilege changes and access to customer data, are logged centrally in the SIEM.

## Log retention

Audit logs are retained for at least one year, with the most recent 90 days immediately searchable. Logs are stored in write-once storage to prevent tampering.

## Monitoring and alerting

The Security Operations team monitors alerts 24x7. Alerts for critical detections page the on-call analyst.

## Time synchronization

All systems synchronize clocks with NTP to ensure accurate log timestamps.
