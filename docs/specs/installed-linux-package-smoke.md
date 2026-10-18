# Installed Linux desktop package smoke

Authority: `distribution` release gates in `docs/FEATURE-MATRIX.md` and `docs/BUILD-AND-RELEASE.md`. Existing `scripts/check-linux-desktop-packages.py` verifies extracted deb, RPM and AppImage payloads; this slice adds actual `.deb` installation on the ephemeral Ubuntu build runner.

## Objective and boundaries

- After the Linux desktop package build and extracted-payload check, install the one produced `.deb` with the runner's package manager. A failed install, missing dependency or wrong installed path fails the workflow. Do not substitute extraction for installation or repair package metadata during the smoke.
- Confirm the package reported by `dpkg-deb --field` is installed, `/usr/bin/moleapi` exists, and `/usr/lib/MoleAPI` owns runtime resources. Run the existing headless script-worker check and exact bundled Java/Dubbo checks against these installed paths. Those checks launch the actual installed worker/JVM; no ambient development executable or runtime path can satisfy them.
- Installation only runs in GitHub Actions on the Linux desktop job's disposable Ubuntu host after the package is built. The helper refuses local/non-Linux invocation before any package-manager mutation. It does not install on a developer machine, auto-publish an artifact or change macOS/Windows packages.
- Use an exact single `.deb` under the supplied bundle tree; reject zero/multiple matches and unexpected package metadata. Bound the install command to five minutes and each installed-resource smoke to its existing worker/JVM timeouts. Preserve logs on failure without printing credentials.

## Acceptance

- Synthetic unit tests prove local invocation and ambiguous package selection cannot call the package manager; a mocked CI invocation checks the command and installed-path assertions. `python3 -m py_compile` and workflow YAML/actionlint validation pass.
- A fixed-head Linux desktop CI job installs its freshly built deb and passes all three checks from `/usr/bin` and `/usr/lib/MoleAPI`. Extraction, RPM and AppImage gates still run. This is Linux installation evidence only; graphical launch, macOS/Windows installer operation, signing/notarization and final release remain open.

## Structure

`scripts/check-installed-linux-deb.py` owns the guarded install and installed-path checks, `scripts/test_installed_linux_deb.py` owns safe process fixtures, and `.github/workflows/build.yml` calls it after the existing extracted package smoke. `docs/BUILD-AND-RELEASE.md` records the fixed-head result and limits.
