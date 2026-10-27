# Security policy

## Reporting a vulnerability

Please do **not** open a public issue, discussion or pull request for a
security problem.

Report it privately through GitHub's
[private vulnerability reporting](https://github.com/Ourea-Technologies/ensemble-weather-rust/security/advisories/new)
("Report a vulnerability" on the Security tab). Include what you found, how to
reproduce it, and the SDK version (`ensemble_weather::VERSION`).

We acknowledge reports within three working days, keep you updated while we
fix the problem, and credit you in the advisory unless you prefer otherwise.

## Supported versions

Security fixes go to the latest release. Before 1.0, that is the latest 0.x
minor version.

## What the SDK guarantees

- The API key is sent only in the `X-API-Key` header, only to the configured
  base URL. A redirect to any other origin is followed without it.
- The key never appears in errors, `Display` or `Debug` output; a message
  from the server that echoes it is redacted.
- TLS verification is always on (rustls, with the platform's verifier), and
  there is no option to turn it off. A failed certificate check is never
  retried.
- Response bodies are bounded: a body over 64 MiB is an error, not an
  unbounded allocation.
- Responses are decoded with serde into fixed types only; nothing is
  evaluated and there is no polymorphic deserialisation.
- The crate forbids `unsafe` code, and CI runs `cargo audit` and
  `cargo deny` (advisories, licences, bans) on every change and weekly.

Vulnerabilities in the Ourea API itself can be reported the same way.
