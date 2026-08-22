# Contributing safely

Keep changes small and preserve Agent HUD's read-only, local, native-Windows
scope. The primary Codex App remains the owner of approvals, user input,
turns, and session control.

## Use an isolated worktree

Do not develop on `main` or in a worktree being used by another task. From a
clean checkout, create a branch and worktree together:

```powershell
git fetch origin
git worktree add .worktrees/issue-<number> -b codex/issue-<number>-<short-name> origin/main
Set-Location .worktrees/issue-<number>
```

Keep the worktree path out of unrelated tasks. Before editing, confirm the
branch and working tree:

```powershell
git status --short --branch
git worktree list
```

When the work is integrated, remove only the worktree you created:

```powershell
Set-Location <repository-root>
git worktree remove .worktrees/issue-<number>
```

## Make and verify a change

1. Read `AGENTS.md` and the relevant requirements/architecture documents.
2. Read the issue's goal, scope, non-goals, and ownership. Do not modify
   files outside that ownership without an explicit scope change.
3. Inspect nearby code and tests before editing. For protocol changes, use
   deterministic fixtures and fail closed when passive observation is not
   proven safe.
4. Run the applicable checks:

   ```text
   cargo fmt --check
   cargo check
   cargo clippy --all-targets -- -D warnings
   cargo test
   ```

5. Review the diff and confirm that generated files, secrets, and unrelated
   work are absent.

For documentation-only changes, the Rust checks are still useful when the
working tree is otherwise clean, but the important review is link accuracy,
command accuracy, and agreement with the current implementation.

## Commit and hand off

Use a focused commit message, then push the issue branch:

```powershell
git add README.md docs
git commit -m "docs: add user and architecture guide"
git push -u origin codex/issue-<number>-<short-name>
```

Open a draft pull request that states the issue, scope, checks run, and any
unverified runtime evidence. Do not merge from the task worktree.
