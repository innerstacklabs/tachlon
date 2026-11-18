# How this was built

This project was built by Dave Remy, who owns the product and decides what it is, together with a team of AI agents that wrote, tested and reviewed the code. The first commit is from 2026-10-20. You will meet a few names and ids from that work in code comments, decision records and tests. This page says what they are.

## Names

- **karpathy** is the agent that acts as product owner and engineering lead. It turns Dave's decisions into tasks, sets priorities, and rules on questions the dev team raises.
- **The dev team** (`devteam-1`) is the group of agents that takes a task, writes the change, and checks it, with other agents as reviewers. A person (Dave) reads and approves the results that matter to the product.
- **lifeos** is Dave's personal system, the tooling in which the agents run. It is not part of this project and is not in this repository. It appears in a few comments where a script or a task came from it.
- **sagan** is the agent that does research: the market and palette studies that some decisions cite.
- **astra** is a review model, an AI model used to read a plan or a change and find faults; comments such as "astra's two counterexamples" credit a fault it found.
- **The hub, the mini and mbp** are the machines the work ran on: the hub is the Linux server where the agents ran and built, the mini a Mac mini used for benchmark runs, and mbp the MacBook Pro the screencasts were recorded on (it has the GPU the frame-rate guard needs). They belong to the maintainers; no contributor needs them (`docs/MEASURING.md` runs on any machine).

## Ids

- **A ruling id**, such as `A devteam-1-1337`, is a numbered product owner decision: a written answer from karpathy to a question the dev team raised, kept so the same question is not decided twice. The number is its place in the team's sequence.
- **An issue id**, such as `gs#96`, `tl#1094` or `to#1`, is an issue or task number in the tracker the work was planned in. The prefix says which tracker: `gs` is the project's earlier tracker (it ran under an earlier working name, see decisions 0006 and 0013), `tl#N` is an issue of the private tracker the work was planned in (it was `gantt#N` until 2026-11-17, so an older `gantt#N` is the same number), `to#N` is an issue or pull request of this repository, and the other prefixes are the agent team's own task lists. You cannot open the issue behind an id of the earlier or private trackers. The comment or commit message around the id says what was decided.
- **A decision record** is a file in `docs/decisions/`. Each records one choice, who made it, and why. They are numbered in the order they were made. Where a later record replaces part of an earlier one, the earlier one says so in its text.

## In numbers

The first commit is from 2026-10-20, in the earlier code repository. Measured on 2026-11-17 with `gh pr list -R <repository> --state merged --limit 2000 --json number --jq length`: 1299 merged pull requests in the earlier code repository, and 52 in the repository the work moved to on 2026-11-16, 1351 in all. This repository's own count starts at 1 and is not included.

## What this means for you

You can ignore the ids when you read the code: the comment next to each one says what the code does. When a record says "decided by Dave" the choice is Dave's. When it names karpathy, an agent made the ruling. If you want to know why a rule exists, start with `docs/decisions/` and `docs/ARCHITECTURE.md`.
