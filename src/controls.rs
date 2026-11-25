//! Maps questions and policies to common control frameworks (SOC 2 TSC, ISO/IEC 27001:2022 Annex A, NIST CSF 2.0).

use crate::text;
use std::collections::HashSet;
use std::sync::LazyLock;

pub struct Topic {
    pub id: &'static str,
    pub keywords: &'static str,
    pub soc2: &'static str,
    pub iso27001: &'static str,
    pub nist_csf: &'static str,
}

pub const TOPICS: &[Topic] = &[
    Topic {
        id: "access_control",
        keywords: "access mfa authentication password sso privilege role account revoke review termination",
        soc2: "CC6.1, CC6.2, CC6.3",
        iso27001: "A.5.15, A.5.18, A.8.5",
        nist_csf: "PR.AA",
    },
    Topic {
        id: "encryption",
        keywords: "encryption encrypted tls key kms aes cryptography transit rest",
        soc2: "CC6.1, CC6.7",
        iso27001: "A.8.24",
        nist_csf: "PR.DS",
    },
    Topic {
        id: "data_lifecycle",
        keywords: "retention deletion classification residency region removable media",
        soc2: "C1.1, C1.2",
        iso27001: "A.5.12, A.5.33, A.8.10",
        nist_csf: "PR.DS",
    },
    Topic {
        id: "logging_monitoring",
        keywords: "log logging audit siem monitoring alert retention tampering ntp",
        soc2: "CC7.2, CC7.3",
        iso27001: "A.8.15, A.8.16, A.8.17",
        nist_csf: "DE.CM",
    },
    Topic {
        id: "incident_response",
        keywords: "incident breach notification response tabletop post-incident",
        soc2: "CC7.4, CC7.5",
        iso27001: "A.5.24, A.5.25, A.5.26, A.5.27",
        nist_csf: "RS.MA, RS.CO",
    },
    Topic {
        id: "business_continuity",
        keywords: "backup restore recovery rpo rto disaster failover availability continuity",
        soc2: "A1.2, A1.3",
        iso27001: "A.5.29, A.5.30, A.8.13, A.8.14",
        nist_csf: "RC.RP",
    },
    Topic {
        id: "vendor_management",
        keywords: "vendor third-party subprocessor supplier due diligence contract processing agreement",
        soc2: "CC9.2",
        iso27001: "A.5.19, A.5.20, A.5.21, A.5.22",
        nist_csf: "GV.SC",
    },
    Topic {
        id: "secure_development",
        keywords: "code review change pipeline static analysis dependency scanning environment",
        soc2: "CC8.1",
        iso27001: "A.8.25, A.8.28, A.8.31, A.8.32",
        nist_csf: "PR.PS",
    },
    Topic {
        id: "vulnerability_management",
        keywords: "vulnerability penetration pentest remediation patch scanning",
        soc2: "CC7.1",
        iso27001: "A.8.8",
        nist_csf: "ID.RA",
    },
    Topic {
        id: "hr_security",
        keywords: "background check training awareness confidentiality employee onboarding disciplinary",
        soc2: "CC1.4, CC2.2",
        iso27001: "A.6.1, A.6.2, A.6.3, A.6.4",
        nist_csf: "PR.AT",
    },
];

static TOPIC_TERMS: LazyLock<Vec<HashSet<String>>> = LazyLock::new(|| {
    TOPICS
        .iter()
        .map(|t| text::terms(t.keywords).into_iter().collect())
        .collect()
});

/// Topics whose keywords appear in `terms`, strongest match first.
pub fn classify(terms: &[String]) -> Vec<&'static Topic> {
    let set: HashSet<&str> = terms.iter().map(String::as_str).collect();
    let mut hits: Vec<(usize, usize)> = TOPIC_TERMS
        .iter()
        .enumerate()
        .map(|(i, kw)| (i, kw.iter().filter(|k| set.contains(k.as_str())).count()))
        .filter(|h| h.1 > 0)
        .collect();
    hits.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    hits.into_iter().take(2).map(|(i, _)| &TOPICS[i]).collect()
}

pub fn topic_terms(i: usize) -> &'static HashSet<String> {
    &TOPIC_TERMS[i]
}

pub fn describe(t: &Topic) -> String {
    format!(
        "SOC2 {}; ISO27001 {}; NIST CSF {}",
        t.soc2, t.iso27001, t.nist_csf
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_questions() {
        let ids = |q: &str| {
            classify(&text::terms(q))
                .iter()
                .map(|t| t.id)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            ids("Is customer data encrypted in transit with TLS?")[0],
            "encryption"
        );
        assert_eq!(
            ids("Do you enforce MFA for all accounts?")[0],
            "access_control"
        );
        assert_eq!(ids("What are your RPO and RTO?")[0], "business_continuity");
        assert!(ids("What is your favourite colour?").is_empty());
    }
}
