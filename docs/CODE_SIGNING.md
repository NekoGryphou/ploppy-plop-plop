# Windows code-signing plan

## Project decision

Decky My Rig will be distributed directly through GitHub Releases, not through
the Microsoft Store. The project does not intend to pay for a commercial
certificate or subscription. Public Authenticode signing is deferred until the
project is mature enough to apply to SignPath Foundation.

Until then:

- development and alpha artifacts may be unsigned and must be labelled as such;
- users should be told to expect Microsoft Defender SmartScreen warnings;
- SHA-256 checksums and GitHub build-provenance attestations must accompany
  published artifacts;
- the automatic host updater must continue rejecting unsigned installers;
- never weaken signature verification merely to publish an alpha build; and
- do not distribute a self-signed root certificate to public users.

Self-signed certificates are acceptable only for controlled local/VM testing.
Microsoft states that they provide the same SmartScreen reputation behavior as
unsigned files unless users manually trust the certificate.

## Intended free production route

[SignPath Foundation](https://signpath.org/) provides free code signing for
eligible open-source projects. Apply through its
[application page](https://signpath.org/apply.html) after the repository is
public, actively maintained, documented, and has an existing release.

Before applying, review the current
[SignPath Foundation conditions](https://signpath.org/terms.html). Important
requirements currently include an OSI-approved license, no proprietary project
components, repository and signing-account MFA, maintained/released software,
documented functionality and privacy behavior, defined signing roles, manual
release approval, and verifiable automated builds. Requirements may change, so
the linked source is authoritative.

SignPath uses a managed signing pipeline and HSM-held key. It does not provide a
PFX private key to store in GitHub. When accepted, replace the current PFX-based
release-signing step with the integration specified by SignPath's current
[build-system documentation](https://docs.signpath.io/build-system-integration).

## Current PFX workflow

The existing release workflow supports a conventional Authenticode PFX through
these `production` environment secrets:

- `WINDOWS_CERTIFICATE_BASE64`: Base64 encoding of the complete PFX file.
- `WINDOWS_CERTIFICATE_PASSWORD`: password protecting that PFX.

These secrets are intentionally not configured. They are relevant only if the
project later receives a suitable PFX certificate. Never commit a PFX, password,
private key, Base64 certificate secret, or exported signing material.

## Authoritative references

- [Microsoft: SmartScreen reputation for Windows app developers](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation)
- [Microsoft: code-signing options for Windows app developers](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options)
- [SignPath Foundation: free signing for open source](https://signpath.org/)
- [SignPath Foundation: eligibility and policy conditions](https://signpath.org/terms.html)
- [SignPath Foundation: application](https://signpath.org/apply.html)
- [SignPath: build-system integration](https://docs.signpath.io/build-system-integration)
- [GitHub: environment and repository secrets](https://docs.github.com/en/actions/how-tos/write-workflows/choose-what-workflows-do/use-secrets)

Review these sources again at implementation time rather than relying on copied
pricing, availability, or eligibility details.
