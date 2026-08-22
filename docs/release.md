# Release process

This project prepares Windows release artifacts through the manually triggered
`Prepare Windows release` workflow. The workflow does not create a GitHub
Release, publish an artifact, or change repository state beyond the temporary
workflow artifact.

## Release checklist

1. Confirm the intended commit is on `main` and that the working tree is clean.
2. Confirm the version in `Cargo.toml` is the intended release version. The
   workflow input must match it exactly, including any pre-release suffix.
3. Start **Actions → Prepare Windows release → Run workflow** on that commit
   and enter the same version.
4. Wait for the Windows x64 job to pass. Download the artifact named
   `agent-hud-v<version>-windows-x86_64` and inspect the executable,
   `release-manifest.json`, and `SHA256SUMS.txt`.
5. Verify the manifest commit is the intended commit and verify the executable
   checksum before any human-controlled publication step.
6. If publishing is approved, create the GitHub Release manually from the
   intended tag and attach the downloaded ZIP. Publishing is an explicit
   human action and is outside this workflow.

The package is built with `cargo build --locked --release` on
`windows-latest`. The package name, manifest version, selected commit, target,
and checksums are recorded in the artifact so a downloaded package can be
traced to the exact workflow run. Workflow artifacts are retained for 14 days.

## Versioning and repeatability

The workflow accepts one explicit version input and refuses to package when it
does not equal the `agent-hud` package version reported by locked Cargo
metadata. It checks out the exact workflow commit (`github.sha`) and uses the
lockfile, rather than resolving a new dependency graph during the build.

The ZIP is a transport package, not a source of version truth. `Cargo.toml`
and the selected Git commit remain authoritative. Re-running the workflow for
the same commit and version should produce the same named contents; compiler,
runner, and archive metadata can still vary between hosted runner images, so
the manifest and checksums must be checked for every run.

## Failure and rollback behavior

- **Input or version validation fails:** do not retry with a different version
  unless `Cargo.toml` was intentionally changed. Correct the input or commit,
  then run the workflow again.
- **Build or packaging fails:** the workflow uploads no release package. Keep
  any existing published release unchanged, diagnose the failed run, and
  rerun only after the cause is understood.
- **Checksum or manifest is unexpected:** treat the artifact as invalid. Do
  not publish it; discard the downloaded artifact and rerun from the intended
  commit after investigating.
- **A published release must be withdrawn:** use GitHub's normal human-owned
  release controls to mark it as a prerelease or delete the release/tag as
  appropriate to the incident. Do not replace a published asset in place
  without recording the reason and verifying the replacement.

There is no automated rollback because there is no automated publication. A
failed preparation cannot alter an existing release, and a publication error
stops at the manual release step.
