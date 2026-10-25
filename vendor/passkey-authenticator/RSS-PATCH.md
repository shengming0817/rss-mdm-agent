# RSS passkey authenticator patch

Source: crates.io passkey-authenticator 0.6.0, archive SHA-256
`5fb7e1cb79a932ac3a5ceb0fa73b25a6028ef1bd69a007441e6cce9ecf2e6e0f`.
Upstream: https://github.com/1Password/passkey-rs/tree/1606717bd9888b629905d435bb876b24b1aaddb5
License: MIT OR Apache-2.0 (included).

#2928: set BE before authenticator data serialization/signing in make_credential
and get_assertion. All credentials admitted by this product are backup eligible;
Explicitly remove upstream default BS before serialization/signing because this product has no backup publication path. No response
is modified after signing. Core crypto/types remain registry dependencies.
Remove this source patch when a pinned upstream release supports this policy.

The normalized manifest has a standalone workspace boundary for nested worktree tooling.
