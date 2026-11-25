---
title: Data Protection and Encryption Standard
owner: Security Engineering
last_reviewed: 2026-04-02
---

# Data Protection and Encryption Standard

## Encryption at rest

All customer data is encrypted at rest using AES-256. Database volumes, object storage and backups use encryption managed by the cloud provider key management service.

## Encryption in transit

All data in transit over public networks is encrypted with TLS 1.2 or higher. Legacy protocols such as SSL v3 and TLS 1.0 are disabled.

## Key management

Encryption keys are managed in AWS KMS. Customer-managed keys are rotated annually, and access to key administration is restricted to the security team.

## Data classification

Data is classified as Public, Internal, Confidential or Restricted. Customer data is classified as Confidential by default.

## Data retention and deletion

Customer data is retained for the duration of the contract and deleted within 30 days of contract termination. Deletion is certified on request.

## Removable media and personal devices

Customer data is never stored on removable media or personal devices.

## Data residency

Customer data is hosted in the AWS eu-west-1 and us-east-1 regions. Customers choose their hosting region at onboarding.
