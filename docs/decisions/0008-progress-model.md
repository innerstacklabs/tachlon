# The progress model

Status: accepted. Chosen in the "Plan: workflow 2, update progress" comment on gs#703 (story 1); this record states the four picks the later stories build on.

## Context

Journey J8 (PRODUCT.md) updates a schedule with progress: a data date says "today", work that has not started cannot start before it, and a task can be started or finished. The stories of the update-progress workflow add these in steps: story 1 the data date, story 2 the actual dates and remaining duration. Each step needs the same answers to four questions, so they are written down once.

## Decision

1. **Percent complete is derived.** A task stores its remaining duration; percent is computed from the remaining duration and the duration (the duration type of percent complete): percent = (duration - remaining) / duration. A milestone has no percent beyond done or not done. Nothing stores a percent the engine could contradict.
2. **Retained logic for an out-of-sequence actual (the P6 default).** When a task has an actual start before its predecessors have finished, the dependencies stay in force: the task's remaining work is scheduled after the predecessors, as for any other task. Progress override (the remaining work ignores the predecessors that are not finished) is a later option, not built now.
3. **The float of a finished task is 0.**
   3a. **A finished task is not critical, as in P6; its float stays 0** (amended 2026-10-29, gs#825). The remaining work decides the critical path, so a task with an actual finish is not drawn, marked or listed as critical and Critical only leaves it out. `TaskDates::finished` carries it; a started task with remaining work stays eligible.
4. **The float of a started task is measured from its early finish**: the working minutes of its calendar from the early finish to the late finish. An unstarted task keeps the float measured from its early start to its late start. Free float (`Schedule::free_floats`, gantt#2084) follows points 3 and 4: 0 for a finished task, with `TaskDates::finished` set, and measured from the early finish for a started one.

The data date itself (story 1) is a field of the schedule. The forward pass starts an unstarted task no earlier than the data date; a started task is not floored by it. Changing the data date moves every unstarted task, so it is solved by a full `compute`, not a cone.

## Consequences

- Stories 2 to 4 add actuals and remaining duration without reopening these four answers; a story that needs a different answer amends this record first.
- A schedule with no data date schedules exactly as before.
- The data-date floor adds one comparison to the forward pass of every task. The frame-time read on a GPU machine (docs/MEASURING.md, "Measuring frame times") checks that it costs nothing a person can see.
