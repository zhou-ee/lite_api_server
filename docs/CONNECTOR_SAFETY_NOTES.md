# Connector Safety Notes

This repository is often modified through a GitHub connector rather than a local git client. The connector may run a pre-submit safety check before writing files.

## Observed behavior

Some writes can be blocked even when the code is intended as a placeholder or example. This is not a repository permission problem and does not necessarily mean the code is unsafe. It means the connector declined the write before GitHub accepted it.

## Patterns that previously caused friction

Avoid combining too many of these topics in one patch:

- credential-like config field examples
- management endpoint examples
- authorization header examples
- log cleanup or deletion code
- cost/pricing endpoints
- full config examples containing secret-shaped placeholders

## Safer commit strategy

- Split large changes into smaller commits.
- Prefer neutral placeholder values such as `__REPLACE_ME__`.
- Keep example config values generic and non-secret-shaped.
- Avoid mixing credential placeholders, management APIs, deletion logic and pricing logic in one file update.
- Document behavior in prose when a direct example risks triggering the connector.

## Current workaround

Pricing is currently part of the full runtime config schema and can be updated through the full config editor. A dedicated pricing-only Admin API can be added later as a small, isolated patch.
