# Security policy

## Scope

The contracts in this repository are **test fixtures** that fail on purpose.
They are not audited and must not be used to hold real value. Intentional
failure behaviour (e.g. the panic in `unchecked_add`) is not a vulnerability.

We do want to hear about:

- A fixture contract that behaves *unsafely* beyond its documented failure
  (e.g. an authorization bypass in `auth-guard`)
- Scripts or CI that could leak credentials or keys
- Anything in the catalog that could mislead SorobanLens into giving
  dangerous advice

## Reporting

Please **do not open a public issue**. Use GitHub's
[private vulnerability reporting](https://docs.github.com/en/code-security/security-advisories/guidance-on-reporting-and-writing-information-about-vulnerabilities/privately-reporting-a-security-vulnerability)
on this repository, or contact the maintainers at
**`<security contact to be set by maintainers>`**.

We aim to acknowledge reports within 5 working days.
