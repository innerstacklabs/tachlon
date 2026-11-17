# A Gantt component for developers

Status: accepted. Decided by Dave, 2026-11-17 09:17 to 09:19 (terminal; gantt#2125). Supersedes [decision 0005](0005-users-first-component-later.md).

## Context

Decision 0005 put end users first: the app for people who read and review P6 schedules was the product, and the developer component came later. Dave changed that on 2026-11-17, in three messages:

> "tachlon is a scheduling/gantt component similar to ... fully featured, blazingly fast, no strings attached open source" <!-- verbatim -->

> "we changed our focus" <!-- verbatim -->

> "we need to remove any vestiges of [the old name]?"

The license half of this was already decided: decision 0014 made Tachlon MIT with no paid tier.

## Decision

1. **Tachlon is a Gantt and scheduling component for developers.** It sits in the category of Bryntum, DHTMLX and Syncfusion.
2. **It is fully featured, blazingly fast, and open source with no strings.** Decision 0014 holds the license.
3. **Opening P6 XER and MS Project files is a feature, not the audience.** The people Tachlon is built for are developers who put a Gantt chart in their own product.
4. **"Blazingly fast" is the speed wording everywhere** (README, site, book, package description, a pull request, a post). It replaces any "fastest" wording. A speed number still comes with its bench, machine and command (`REVIEW.md`).
5. **Decision 0005 is superseded.** Its order (users first, the component later) is replaced by items 1 to 3. Its item 4 (the license) was already superseded by decision 0014. Its item 3, that the app uses only the public API, stays as a rule: the component's public API is now the product's surface, and decision 0007 cites it.

## Consequences

- A page, a doc or a plan that calls the app the product, or aims Tachlon at P6 readers first, is fixed where found.
- A claim of "the fastest" anywhere is wrong and is fixed where found.
- Decision 0005 keeps its body as written, apart from the old name in its two gs#1044 notes; its status line names this record.
