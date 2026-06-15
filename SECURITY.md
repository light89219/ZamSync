# Security Policy

ZamSync handles event replication between nodes in environments where data may
be sensitive -- including healthcare records, audit logs, and access-controlled
payloads. Security is taken seriously. If you find a vulnerability, please
follow the process below so it can be addressed responsibly before any public
disclosure.

---

## Supported Versions

Only the latest released version receives security fixes. Older versions are not
patched.

| Version | Supported |
|---------|-----------|
| Latest stable (`crates.io`) | Yes |
| Older releases | No -- please upgrade |

---

## Reporting a Vulnerability

**Do not open a public GitHub issue for security vulnerabilities.**

Use one of the two channels below depending on severity:

### Preferred -- GitHub Private Vulnerability Reporting

Open a private security advisory directly on GitHub. This keeps the report
confidential until a fix is ready and allows coordinated disclosure.

[Report a vulnerability](https://github.com/Etoile-Bleu/ZamSync/security/advisories/new)

GitHub routes the report to the maintainer only. No other users can see it.

### Alternative -- Email

If you prefer email or if the GitHub flow does not work for you:

**tarto6351@gmail.com**

Please use the subject line `[ZamSync] Security vulnerability report` and
include as much detail as possible (see template below).

---

## What to Include in Your Report

The more detail you provide, the faster the issue can be triaged and fixed.

```
Component:     (e.g. WAL encryption, mTLS transport, access control, CLI)
Version:       (output of `zamsync --version` or crates.io version)
Severity:      (your assessment: critical / high / medium / low)
Attack vector: (local / network / physical)
Description:   (what the vulnerability is)
Reproduction:  (step-by-step to trigger it)
Impact:        (what an attacker can achieve)
Suggested fix: (optional, but very welcome)
```

---

## Response Timeline

| Step | Target |
|------|--------|
| Acknowledge receipt | Within 48 hours |
| Initial severity assessment | Within 5 business days |
| Patch ready for review | Within 30 days for high/critical, 90 days for medium/low |
| Public disclosure | Coordinated with the reporter after patch release |

These are targets, not guarantees. Complex vulnerabilities or those requiring
protocol-level changes may take longer.

---

## Coordinated Disclosure

ZamSync follows a coordinated (responsible) disclosure model:

1. Reporter submits the vulnerability privately.
2. Maintainer acknowledges and assesses severity.
3. A fix is developed and reviewed in a private fork or security advisory branch.
4. A patched release is published to crates.io and GitHub Releases.
5. A GitHub Security Advisory is published simultaneously with the release.
6. The reporter is credited in the advisory and in `ACKNOWLEDGEMENTS.md`
   (unless they prefer to remain anonymous).

The reporter and the maintainer agree on a disclosure date before the patch is
released. The default embargo is 90 days from the initial report, or sooner if
both parties agree.

---

## Scope

### In scope

- **WAL encryption** -- ChaCha20-Poly1305 nonce reuse, key derivation weaknesses,
  plaintext leakage, authentication bypass
- **mTLS transport** -- certificate validation bypasses, rogue node acceptance,
  CA impersonation, TLS downgrade attacks
- **Access control** -- `--policy own` bypass, cross-node event leakage, privilege
  escalation between peers
- **Wire protocol** -- deserialization vulnerabilities, frame parsing issues,
  denial-of-service via malformed frames, integer overflows in frame size
  handling
- **WAL integrity** -- CRC bypass, tampered record acceptance, compaction
  tombstone manipulation
- **CLI** -- command injection via flags or arguments, path traversal in data
  directory handling
- **REST API** -- authentication bypass on the embedded HTTP server, SSRF,
  injection via `POST /submit`

### Out of scope

- Vulnerabilities in third-party dependencies (report those upstream; we will
  update the dependency promptly on notification)
- Attacks that require physical access to the node's storage medium after the
  node has been compromised at the OS level
- Issues in unreleased branches or experimental features not in the stable
  release
- Social engineering attacks against the maintainer
- Denial of service via legitimate resource exhaustion (e.g. submitting very
  large payloads within documented limits)

---

## Safe Harbor

ZamSync is an open-source project maintained by an individual. Good-faith
security research is welcome and will not result in legal action.

Specifically: if you discover a vulnerability while using ZamSync legitimately,
report it responsibly following this policy, and do not exploit it beyond what
is necessary to demonstrate the issue, you will not face any legal threat from
this project.

---

## Credits

Researchers who responsibly disclose vulnerabilities are credited in the
[GitHub Security Advisory](https://github.com/Etoile-Bleu/ZamSync/security/advisories)
for the corresponding issue and, with their consent, in
[`ACKNOWLEDGEMENTS.md`](ACKNOWLEDGEMENTS.md).
