## cli:standalone-activity-client
📖 ~/AGENTS.md
📖 cli:standalone-activity-client

Based on the git status and recent commits, this branch (`standalone-activity-client`) implements **standalone activity CLI commands** for the Temporal CLI.

**What's been done:**
- Activity command definitions added
- Implementation of standalone activity CLI commands
- Tests for the command structure
- Code review feedback addressed

**What's currently in progress (untracked files):**
- `.gitattributes`, `.task/`, `AGENTS.md`, `CLAUDE.md` — these appear to be local/config files

**To assess what remains to complete this feature**, I'd need to:
1. Review the existing implementation to understand the scope
2. Check if there are any TODOs or incomplete functionality
3. Verify tests pass and coverage is adequate
4. Determine if there's a PR or issue describing the full requirements

Would you like me to explore the codebase to give you a more detailed assessment of what's left?

## documentation:standalone-activity-encyclopedia-entry
**Summary of this task:**

This branch (`standalone-activity-encyclopedia-entry`) adds documentation for **Standalone Activities** — a Temporal feature where an Activity Execution is started directly by a Client without a Workflow.

**What's been done:**
- Added a new encyclopedia entry `standalone-activity.mdx` explaining the concept
- Updated related docs (`activities.mdx`, `activity-execution.mdx`, `detecting-activity-failures.mdx`) with cross-references
- Added the new page to `sidebars.js`

**To complete this task:**
1. Review the content for accuracy and completeness
2. Run any configured linters/formatters and fix issues
3. Build/test the docs site to ensure no broken links
4. Create a PR to main

Would you like me to proceed with any of these steps?

## oncall:gather
📖 ~/AGENTS.md
📖 oncall:gather

I don't have enough context to know what "this task" refers to. You haven't specified a task yet in this conversation.

Could you clarify what task you'd like me to summarize and assess?

## saas-cicd:standalone-activity-dashboard
Now I have a good understanding of the task.

## Task Summary: Standalone Activity Dashboard

**What it is:** Building a Grafana dashboard to monitor "standalone activities" — a new Temporal feature (CHASM-based activities that can be invoked directly by clients without a workflow).

**What's been done (42 commits):**
1. Created dashboard JSON files (`standalone-activity-v0.json`, `standalone-activity-v1.json`)
2. Added panels for: Service Operations (Frontend/History APIs), Task Processing (dispatch latency, RPS, errors), Activity Outcomes (success/fail/timeout), Activity Latency
3. Added filters: operation patterns, task patterns, workflow type filter for `__temporal_standalone_activity__`
4. Added documentation describing the metrics lifecycle
5. Added a "By namespace" row for OSS cells

**To complete this task:** Based on the commit history, the dashboard appears largely complete. Remaining work might include:
- Merging the PR to `main` (the branch is ahead of `origin/dan/standalone-activity-dashboard` by 2 commits)
- Any final review feedback or polish
- Potentially adding more panels or refining existing ones based on usage

Would you like me to look at the current diff against main, or investigate any specific aspect?

## saas-temporal:sa-cogs
📖 ~/AGENTS.md
📖 saas-temporal:sa-cogs

## Task Summary

You're working on a **COGS (Cost of Goods Sold) analysis** comparing **Standalone Activity (SA)** versus **Single-activity Workflow (SAW)** in Temporal Cloud. The goal is to estimate how much cheaper SA is relative to SAW across various compute resources.

## Current State

The document already contains:
- Methodology framework (weighted sum of resource costs)
- Detailed persistence operation analysis with preliminary ratios
- Data ingress/egress analysis (R = 1/2)
- Defined scenarios to test

## What's Missing (TODOs)

1. **At-rest data storage section** - currently empty; need to analyze LPWAL vs HEWAL storage tier costs for SA vs SAW

2. **LPWAL storage tier question** - document has a TODO asking whether SA's LPWAL data is also rapidly moved to cheaper storage (like SAW's HEWAL)

3. **Metrics and logs emissions** - mentioned as differing between SA/SAW but not analyzed

4. **Actual measurements/validation** - the ratios shown are "naive" theoretical estimates; completing the analysis likely requires empirical validation against real workloads

5. **Weighting scheme** - determining relative costs ($w_i$) to combine per-resource ratios into an overall COGS ratio

## To Complete

1. Research LPWAL storage tiering behavior
2. Fill in at-rest data storage analysis
3. Analyze metrics/logs cost differences
4. Potentially run the defined scenarios against test cells to validate theoretical estimates
5. Either obtain relative cost weights or present sensitivity analysis under different weighting assumptions

## saas-temporal:sa-status
📖 ~/AGENTS.md
📖 saas-temporal:sa-status

I don't have context about what "this task" refers to. You're on a branch called `sa-status`, but I don't know what work is intended for this branch.

To understand what this task involves, I could:
1. Look at any existing changes or commits on this branch compared to `main`
2. Check for any related issues, PRs, or documentation
3. Have you tell me what the task is

Would you like to clarify what task you're referring to, or should I investigate the branch to see what work has been done?

## saas-temporal:sa-validation-report
📖 ~/AGENTS.md
📖 saas-temporal:sa-validation-report

I don't have enough context to know what "this task" refers to. You're on a branch called `sa-validation-report`, but I don't know what the task associated with this branch is.

To find out, I could:
1. Look at any existing work on this branch (diff against main)
2. Search for related issues/tickets
3. Check if there's a PR description or related documentation

Would you like me to investigate the branch to understand what the task involves?

## saas-temporal:technical-field-enablement-session
📖 ~/AGENTS.md
📖 saas-temporal:technical-field-enablement-session

## Task Summary

You're preparing for a **Technical Field Enablement Session** on Friday, Feb 20 at 9am Pacific, where you and Phil will present Standalone Activities to GTM (sales) and DevSuccess teams.

### Completed
- Planning demo approach (Python sample with UI)
- Python sample instructions
- Go sample instructions
- Local UI setup

### Remaining Tasks

1. **Cloud cell demo setup** — Figure out what's needed to demo against the `a2dd6` test cloud cell. Determine what server commit is deployed there.

2. **Polyglot demo** (optional but cool) — Worker in Python, client in Go (or vice versa).

3. **Additional technical content** — Decide what deep-dive content to add after Phil's slide 9, beyond what he's already covering. Current notes mention:
   - Worker code is identical to workflow activity code
   - Easy to switch from SA to Workflow

### What it would take to complete

1. **Cloud cell investigation**: Query the oncall repo or use Temporal Cloud tooling to find the server version on `a2dd6` and verify the Standalone Activities feature is available there.

2. **Polyglot demo**: Modify existing samples to have cross-language client/worker, test it works.

3. **Content prep**: Review Phil's slides 7-9, prepare talking points and any additional slides for the deep-dive section.

Would you like me to start on any of these?

## temporal:chasm-not-found
📖 ~/AGENTS.md
📖 temporal:chasm-not-found

Based on the git status and recent commits, this task appears to be about **fixing "CHASM not found" errors** in the Temporal codebase.

Looking at the recent commits:
- `b99de061b` - "Fix CHASM not found errors"
- `3b88a5db0` - "Fix via ArchetypeDisplayName and convertErrors"

The branch `chasm-not-found` has two commits on top of `main` that address this issue. The work appears to already be done and committed.

To determine what remains (if anything), I'd need to check:
1. Whether there's an open PR for this branch
2. Whether tests are passing
3. Whether there are any remaining review comments or follow-up items

Would you like me to investigate the current state of this work?

## temporal:sa-test-coverage
📖 ~/AGENTS.md
📖 temporal:sa-test-coverage

Based on the git status, you're on branch `sa-test-coverage` with recent commits adding "missing standalone activity integration test coverage." The branch has:

- A modified `.gitattributes`
- An untracked `CLAUDE.md` file
- Recent commits refactoring standalone activity tests (grouping by RPC method, inlining helpers as closures)

**To assess what remains to complete this task**, I'd need to:

1. Understand what "standalone activity" tests exist and what coverage gaps were identified
2. Review what the most recent commit (`086df4ecd`) added
3. Check if there are any remaining coverage gaps or if the task is complete

Would you like me to explore the current state of the standalone activity tests to determine what, if anything, remains to be done?

## wormhole:features2
📖 ~/AGENTS.md
📖 wormhole README
📖 wormhole:features2

Looking at the current branch `features2`, I can see from the recent commits:

1. **50050c5** - Clarify task creation error to state expectation vs reality
2. **a8822cd** - Improve error message when task creation fails to discover worktree
3. **253a798** - Replace .task/plan.md with CLAUDE.md in task worktrees
4. **c2d8b9a** - Migrate .code-workspace folder paths during worktree migration
5. **0c25dc4** - Reorganize into handlers/ and cli/ module directories

This appears to be a **feature branch with multiple improvements** already committed. The work includes:

- **Better error messaging** for task creation failures
- **CLAUDE.md adoption** in task worktrees (replacing .task/plan.md)
- **Worktree migration improvements** (code-workspace path handling)
- **Code reorganization** (handlers/ and cli/ module structure)

**To complete this task**, you would need to:

1. **Merge to main** - The branch has clean status and appears ready. You'd need to:
   - Ensure all tests pass
   - Create a PR (if not already done)
   - Get it reviewed and merged

Would you like me to check if there's an existing PR, run the test suite, or investigate any of these changes in more detail?
