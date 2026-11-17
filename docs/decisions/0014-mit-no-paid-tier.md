# MIT, no paid tier

Status: accepted. Decided by Dave, 2026-11-13 (gantt#1959, gantt#1954). Supersedes the license item of decision 0005.

## Context

Decision 0005 left the license open: AGPL-3.0 plus a commercial license, to be re-decided before the component shipped. Gs#1044 then made the product closed source, with a license key, a trial and a watermark in the component. The component is the `tachlon` package, and the question came back: sell it, or give it away.

## Decision

1. **Tachlon is free and MIT-licensed, with no paid tier.** The repository's `LICENSE` is the MIT license. There is no license key, no trial period and no watermark, and no price anywhere on the site (gantt#1959).
2. **Why.** Tachlon is a contribution, not a business. The economics of software are changing, so Tachlon is given away.
3. **Decision 0005 is superseded for its license item** (item 4, "AGPL-3.0 plus a commercial license"). Its other items, users first and the app using only the public API, stand.

## Consequences

- The package, the app and the site carry no license check. A contribution is covered by the MIT license, and the CLA is gone.
- A claim of a paid, commercial or early-access Tachlon is wrong everywhere it appears and is fixed where found.
