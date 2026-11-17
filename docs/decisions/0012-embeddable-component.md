# The embeddable component: package, location, document and chrome

Status: accepted. Decided by Dave, 2026-10-31 (gs#1080; the plan approved at 13:19, sections 1 to 3).

## Context

The product is sold first as an embeddable component (Dave, 2026-10-31). Decision 0005 put users first and the component later; this record brings the component forward and states the plan's first three sections as ruled, so that later stories cite one contract.

## Decision

1. **The package is named `tachlon`.** It is the only product name this record uses.
2. **The package lives in `packages/gantt/`.**
3. **The document the component reads and writes is the Tachlon document, version 1, as JSON.** One JSON shape is the contract between a host page and the component.
4. **The chrome lives in `apps/chrome/`.** The toolbar, menus and other surrounding interface are an app that uses the package; they are not part of it.

## Consequences

- Stories that build the component, the document or the chrome cite this record for names and places.
- This record comes before the code it governs (decision 0003). No code lands with it.
- Decisions 0005, 0007 and 0011 stand as written, except where this record names a place or a name.

## Amended by decision 0017

### Amendment (gantt#2110, decision 0017, karpathy 2026-11-16)

Items 1, 2 and 4 stand. Item 3 said "One JSON shape is the contract between a host page and the component." There are now two shapes, each with one job:

- *The Tachlon document, version 1,* stays the lossless form the component saves and loads (`toDocument()`, `load({ document })`), as decision 0010 defines it.
- *Host records* are what a host with its own database sends and receives: plain data in, the snapshot out, `results()`, the changes in a commit, `update(records)` and the before-edit payload. They share one vocabulary of field names, units and date forms, set in [decision 0017](0017-host-record-vocabulary.md), and identify records by the host's key ([decision 0016](0016-host-identity.md)).

A story that adds a host-facing surface cites decision 0017 for its names; a story that changes what is saved cites decision 0010.
