# `pay-keystore`

OS secure storage abstraction for Pay. Private keys never touch disk as plaintext — they live in the operating system's native credential store.

## Platform Matrix

| OS | Backend | Auth Method | Key Storage |
|----|---------|-------------|-------------|
| **macOS** | Keychain | Touch ID / password | Secure Enclave / Keychain |
| **Windows** | Credential Manager | Windows Hello / PIN | Windows Data Protection API |
| **Linux** | GNOME Keyring / polkit | Password / fingerprint (with polkit policy) | Secret Service API / kernel keyring |

## Why No Plaintext Key Files?

Most Solana tools store keypairs in `~/.config/solana/id.json`. Pay deliberately avoids this because:

1. Any process running as your user can read that file.
2. Malware, misbehaving extensions, or buggy agents can exfiltrate it.
3. Biometric authorization is a stronger user signal than "this process has my UID."

Pay stores only **metadata** (account names, public keys, labels) in `~/.config/pay/accounts.yml`. The actual secret key bytes are encrypted by the OS and require biometric/password authorization to access.

## Named Accounts

Users can have multiple accounts:

```sh
pay account new work      # Create new keypair, name it "work"
pay account list          # Show all accounts
pay account default work  # Set "work" as default
pay --account work curl https://api.example.com
```

Each account has its own keypair in the OS store. Deleting an account removes the metadata but does not necessarily purge the key from the OS store (depends on platform).

## Linux polkit Setup

GNOME Keyring uses polkit for authorization. Without the policy file, keyring access may fail silently:

```sh
sudo cp rust/config/polkit/sh.pay.unlock-keypair.policy /usr/share/polkit-1/actions/
```

This grants Pay the right to prompt for your password or fingerprint before unlocking the keypair.

## Adding a New Platform Backend

1. Implement the `KeystoreBackend` trait.
2. Add platform detection in `src/lib.rs`.
3. Add the auth flow (biometric prompt, password dialog, etc.).
4. Add CI coverage if possible (many secure storage APIs are hard to test headlessly).
