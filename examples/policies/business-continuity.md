---
title: Business Continuity and Disaster Recovery Policy
owner: Infrastructure
last_reviewed: 2026-03-20
---

# Business Continuity and Disaster Recovery Policy

## Backups

Production databases are backed up daily with point-in-time recovery enabled. Backups are encrypted and replicated to a second region. Backup restoration is tested quarterly.

## Recovery objectives

The recovery point objective (RPO) is 1 hour and the recovery time objective (RTO) is 4 hours for production services.

## Disaster recovery testing

The disaster recovery plan, including regional failover, is tested annually and results are reported to leadership.

## High availability

Production services run across multiple availability zones behind load balancers to tolerate the loss of a single zone.
