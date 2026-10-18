# Installed Linux Desktop Package Smoke Implementation Plan

> **For agentic workers:** Execute each checked step against `docs/specs/installed-linux-package-smoke.md`; never install a package on a local developer host.

**Goal:** Verify the built Linux `.deb` through actual installation and headless installed-resource checks.

**Architecture:** A CI-only Python helper checks environment and package cardinality before invoking apt. It then inspects the installed package and reuses existing script-worker, Java and Dubbo checks with explicit installed paths. The Linux desktop workflow calls the helper after extracted-payload smoke.

**Tech Stack:** Python standard library, GitHub Actions Ubuntu runner, apt/dpkg, existing MoleAPI package-check scripts.

**Spec:** `docs/specs/installed-linux-package-smoke.md`

## Global constraints

- Refuse non-GitHub/non-Linux execution before `sudo` or `apt`.
- Require exactly one produced deb; do not guess or select the first.
- Check `/usr/bin/moleapi` and `/usr/lib/MoleAPI` after actual apt installation.
- Keep extracted deb/RPM/AppImage checks and artifact upload behavior intact.

### Task 1: Guarded installed-package checker

**Files:** `scripts/check-installed-linux-deb.py`, `scripts/test_installed_linux_deb.py`.

- [x] Write failing process-mock tests for local refusal, ambiguous/missing debs, installed-path and command contracts.
- [x] Implement guarded apt install and installed-path smoke using the existing check scripts.
- [x] Run tests, py_compile and local refusal probe with zero host mutation.

### Task 2: Fixed-head workflow gate

**Files:** `.github/workflows/build.yml`, `docs/BUILD-AND-RELEASE.md`, this plan.

- [x] Add the CI helper after extracted Linux package smoke; verify actionlint with its known `macos-15-intel` label false positive excluded and check the diff.
- [ ] Commit/push a draft PR and run the exact-head native workflow. Inspect real apt install and worker/JVM logs and package artifacts before claiming Linux installation acceptance.
