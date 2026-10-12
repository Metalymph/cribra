# Built-in streaming compatibility inventory

Prepared core 0.5.0. Derived from `builtin_portfolio_streaming_compatibility_audit`,
which compiles independent and combined portfolios using compiled matcher and
semantic gates. No regex is reparsed for this inventory. All complete portfolios
fail admission. Whole-source support is unchanged.

| Portfolio | Individually admitted rules | Semantic rejections |
|---|---:|---:|
| Core | 3 | 79 |
| Personal | 0 | 0 |
| Financial | 0 | 0 |
| Combined | 3 | 79 |

The complete matcher-plan counts are in [streaming.md](streaming.md). Semantic
rejections can overlap unsupported matcher plans; do not add these counts.

## Rule-level admission

| Rule ID | Matcher rejection | Semantic rejection |
|---|---|---|
| `aws.access-key-id` | none | byte-dependent prefix |
| `aws.secret-access-key` | unbounded pattern | per-rule prefilter |
| `aws.session-token` | unbounded pattern | per-rule prefilter |
| `aws.temporary-access-key-id` | none | byte-dependent prefix |
| `azure.client-secret` | unbounded pattern | per-rule prefilter |
| `azure.shared-access-signature` | unbounded pattern | per-rule prefilter |
| `azure.storage-account-key` | unbounded pattern | per-rule prefilter |
| `cargo.registry-env-token` | unbounded pattern | per-rule prefilter |
| `cargo.registry-token` | unbounded pattern | per-rule prefilter |
| `cloudflare.account-api-token` | none | byte-dependent prefix |
| `cloudflare.global-api-key` | none | byte-dependent prefix |
| `cloudflare.user-api-token` | none | byte-dependent prefix |
| `composer.bearer-token` | unbounded pattern | none |
| `composer.bitbucket-consumer-secret` | unbounded pattern | per-rule prefilter |
| `composer.forgejo-token` | unbounded pattern | per-rule prefilter |
| `composer.http-basic-password` | unbounded pattern | per-rule prefilter |
| `docker.registry-auth` | unbounded pattern | per-rule prefilter |
| `financial.card-verification-code` | unsupported finite context | none |
| `financial.iban` | unsupported finite context | none |
| `financial.pan` | unsupported finite context | none |
| `gcp.escaped-private-key` | unbounded pattern | per-rule prefilter |
| `gcp.private-key` | unbounded pattern | per-rule prefilter |
| `gcp.private-key-id` | unbounded pattern | per-rule prefilter |
| `generic.api-key` | unbounded pattern | per-rule prefilter |
| `generic.auth-token` | unbounded pattern | per-rule prefilter |
| `generic.authorization-basic` | unbounded pattern | per-rule prefilter |
| `generic.authorization-bearer` | unbounded pattern | per-rule prefilter |
| `generic.database-connection-password` | unbounded pattern | per-rule prefilter |
| `generic.database-password-field` | unbounded pattern | per-rule prefilter |
| `generic.ec-private-key` | unbounded pattern | none |
| `generic.encrypted-private-key` | unbounded pattern | none |
| `generic.openssh-private-key` | unbounded pattern | none |
| `generic.passphrase-field` | unbounded pattern | per-rule prefilter |
| `generic.password-field` | unbounded pattern | per-rule prefilter |
| `generic.pgp-private-key` | unbounded pattern | none |
| `generic.pkcs8-private-key` | unbounded pattern | none |
| `generic.rsa-private-key` | unbounded pattern | none |
| `generic.secret` | unbounded pattern | per-rule prefilter |
| `generic.sensitive-hash` | unbounded pattern | per-rule prefilter |
| `github.app-installation-token` | none | byte-dependent prefix |
| `github.app-refresh-token` | none | byte-dependent prefix |
| `github.app-user-token` | none | byte-dependent prefix |
| `github.classic-pat` | none | byte-dependent prefix |
| `github.fine-grained-pat` | none | byte-dependent prefix |
| `github.oauth-token` | none | byte-dependent prefix |
| `github.stateless-installation-token` | unbounded pattern | none |
| `gitlab.access-token` | none | byte-dependent prefix |
| `gitlab.agent-token` | none | byte-dependent prefix |
| `gitlab.ci-job-token` | none | byte-dependent prefix |
| `gitlab.deploy-token` | none | byte-dependent prefix |
| `gitlab.feature-flag-client-token` | none | byte-dependent prefix |
| `gitlab.feed-token` | none | byte-dependent prefix |
| `gitlab.incoming-mail-token` | none | byte-dependent prefix |
| `gitlab.oauth-application-secret` | none | byte-dependent prefix |
| `gitlab.registration-derived-runner-auth-token` | none | byte-dependent prefix |
| `gitlab.runner-auth-token` | none | byte-dependent prefix |
| `gitlab.scim-token` | none | byte-dependent prefix |
| `gitlab.trigger-token` | none | byte-dependent prefix |
| `gitlab.workspace-token` | none | byte-dependent prefix |
| `gradle.repository-auth-header-value` | unbounded pattern | per-rule prefilter |
| `gradle.repository-password` | unbounded pattern | per-rule prefilter |
| `jwt.signed-compact` | unbounded pattern | none |
| `maven.server-password` | unbounded pattern | per-rule prefilter |
| `mfa.otp-provisioning-secret` | unbounded pattern | none |
| `nats.nkey-private-key` | none | none — admitted |
| `nats.nkey-seed` | none | none — admitted |
| `netrc.password` | unbounded pattern | per-rule prefilter |
| `npm.registry-auth` | unbounded pattern | per-rule prefilter |
| `npm.registry-auth-token` | unbounded pattern | per-rule prefilter |
| `npm.registry-password` | unbounded pattern | per-rule prefilter |
| `nuget.package-source-cleartext-password` | unbounded pattern | per-rule prefilter |
| `personal.it-codice-fiscale` | unsupported finite context | none |
| `personal.pl-pesel` | unsupported finite context | none |
| `personal.uk-nhs-number` | unsupported finite context | none |
| `personal.us-ssn` | unsupported finite context | none |
| `pypi.repository-token` | unbounded pattern | per-rule prefilter |
| `rubygems.api-key` | none | byte-dependent prefix |
| `rubygems.host-api-key` | unbounded pattern | per-rule prefilter |
| `slack.app-level-token` | none | byte-dependent prefix |
| `slack.bot-token` | none | byte-dependent prefix |
| `slack.user-token` | none | byte-dependent prefix |
| `slack.workflow-token` | none | byte-dependent prefix |
| `stripe.live-restricted-key` | none | byte-dependent prefix |
| `stripe.live-secret-key` | none | byte-dependent prefix |
| `stripe.test-restricted-key` | none | byte-dependent prefix |
| `stripe.test-secret-key` | none | byte-dependent prefix |
| `stripe.webhook-secret` | none | byte-dependent prefix |
| `swiftpm.netrc-password` | unbounded pattern | per-rule prefilter |
| `swiftpm.registry-password` | unbounded pattern | per-rule prefilter |
| `swiftpm.registry-token` | unbounded pattern | per-rule prefilter |
| `swiftpm.source-control-token` | unbounded pattern | per-rule prefilter |
| `system.htpasswd-password-verifier` | unbounded pattern | none |
| `system.shadow-password-verifier` | unbounded pattern | none |
| `tailscale.api-access-token` | none | byte-dependent prefix |
| `tailscale.auth-key` | none | byte-dependent prefix |
| `tailscale.oauth-client-secret` | none | byte-dependent prefix |
| `tailscale.scim-key` | none | byte-dependent prefix |
| `tailscale.webhook-key` | none | byte-dependent prefix |
| `telegram.bot-token` | none | none — admitted |
| `wireguard.preshared-key` | unbounded pattern | per-rule prefilter |
| `wireguard.private-key` | unbounded pattern | per-rule prefilter |
