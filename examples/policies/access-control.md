---
title: Access Control Policy
owner: Security Engineering
last_reviewed: 2026-06-15
---

# Access Control Policy

## Authentication

Multi-factor authentication is required for all workforce accounts, including access to email, source control, cloud consoles and the VPN. Phishing-resistant MFA (FIDO2 security keys) is mandatory for administrators.

Single sign-on through our identity provider is enforced for all SaaS applications that support SAML or OIDC.

## Passwords

Passwords must be at least 14 characters long. Password reuse across the last 12 passwords is blocked. Passwords are never shared between employees, and shared service credentials are stored only in the corporate secrets vault.

## Least privilege and role-based access

Access is granted on a least-privilege basis using role-based access control. Production access is restricted to on-call engineers and requires a just-in-time approval that expires after 8 hours.

## Access reviews

User access to production systems and customer data is reviewed quarterly by system owners. Privileged accounts are reviewed monthly.

## Joiners, movers and leavers

Access for terminated employees is revoked within 24 hours of termination. Role changes trigger removal of access that is no longer required within 5 business days.

## Remote access

Remote access to internal systems requires the corporate VPN or zero-trust access proxy, and only from company-managed devices.
