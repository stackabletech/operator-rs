---
name: Pre-Release Workspace Member Updates and Releases
about: This template can be used to track the updates and releases of all workspace members leading up to the next Stackable release
title: "chore: Update and release workspace members for Stackable Release YY.M.X"
labels: ['epic']
assignees: ''
---

<!--
    DO NOT REMOVE THIS COMMENT. It is intended for people who might copy/paste from the previous release issue.
    This was created by an issue template: https://github.com/stackabletech/operator-rs/issues/new/choose.
-->

Part of stackabletech/issues#xxx.

> [!NOTE]
> During a Stackable release we update all dependencies in the `operator-rs`
> repository. After these bumps, each workspace member is released using an
> appropriate SemVer version. Later, each product operator repository can then
> use the updates crates.

Replace the items in the task lists below with the applicable Pull Requests

- [ ] Update Rust version and workflow actions, see below for more details.
- [ ] Update Rust dependencies, see below for more details.
- [ ] Adjust and then verify crate versions using `.scripts/verify_crate_versions.sh`.
- [ ] Push the release tags using `.scripts/tag_and_push_release.sh`.

## Update Rust Version and Workflow Actions

> [!NOTE]
> The PR is usually titled: `chore: Bump Rust version and workflow actions`

1. Adjust the version of the channel in the `rust-toolchain.toml` file. See
   <https://releases.rs>.
2. Adjust the version `RUST_TOOLCHAIN_VERSION` in the workflows:
     - `.github/workflows/build.yml`
     - `.github/workflows/pre_commit.yaml`
     - `.github/workflows/publish-docs.yaml`
3. Add a changelog entry.
4. Update any actions (using the Git commit hash) in the workflows. Hint: Also
   make sure that the `cargo-udeps` action is up-to-date, otherwise the CI might
   report errors.

## Update Rust Dependencies

> [!NOTE]
> This PR is usually titled: `chore: Bump Rust dependencies`

1. Bump minor versions of dependencies in the `Cargo.toml` manifest.
2. Then run the `cargo update` command.
3. Fix any code which needs updating due to the dependency bumps.
4. Locally update any product operator to identify any breaking changes
   downstream.
     - Hint: Use the `[patch."https://github.com/..."]` mechanism to temporarily
       override the dependency.
5. Add a changelog entry if required.

## Adjust and Verify Crate Versions

> [!WARNING]
> Currently, all workspace members use `0.X.Y` versions. This means we can
> introduce breaking changes in any version without needing to bump the major
> level. But we still have the following rules:
>
> - Breaking changes (internally and externally) will bump the minor level of
>   the version, so `0.X.Y` becomes `0.X+1.Y`.
> - All other non-breaking changes will bump the patch level of the version, so
>   `0.X.Y` becomes `0.X.Y+1`.

<!-- markdownlint-disable-next-line MD028 -->
> [!NOTE]
> The PR is usually titled: `chore: Release workspace members`

1. Bump the crate versions in their appropriate `Cargo.toml` manifests.
2. Verify the previous step using `.scripts/verify_crate_versions.sh`.
