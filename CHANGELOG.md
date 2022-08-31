# Changelog

## Unreleased - 2022-XX-YY

- Add support for RoleBindings (https://github.com/kdash-rs/kdash/pull/245)

## [0.3.4] - 2022-08-18

- Add support for Cluster Roles (https://github.com/kdash-rs/kdash/pull/236)
- Add support for decoding secrets (https://github.com/kdash-rs/kdash/pull/242)
- Upgrade libraries

## [0.3.3] - 2022-08-01

- Upgrade libraries
- Add sponsors
- Add support for Roles (https://github.com/kdash-rs/kdash/pull/224)
- Add support for Storage classes (https://github.com/kdash-rs/kdash/pull/222)
- Add support for ARM and aarch64 binaries

## [0.3.1] - 2022-04-06

- Upgrade libraries trying to fix cargo install issue

## [0.3.0] - 2022-02-05

- UI updates (https://github.com/kdash-rs/kdash/pull/157)
- Fix stack overflow error (https://github.com/kdash-rs/kdash/issues/160)
- Color contrast improvements (fix https://github.com/kdash-rs/kdash/issues/162)

## [0.2.7] - 2022-01-20

- Fix crashes when memory and/or cpu usages are higher than 100%
- Improve cache

## [0.2.6] - 2022-01-19

- Fix status color of pods not ready

## [0.2.5] - 2021-12-21

- Fix help screen which was not rendered
- Fix status color of pods not ready
- Update dependencies

## [0.2.4] - 2021-09-27

- Update dependencies
- Fix crash on cargo install

## [0.2.3] - 2021-08-02

- Add support for ReplicationControllers
- Fix issue with table overflow crash

## [0.2.2] - 2021-07-20

- Add support for page up and page down on tables and text views
- Fix crash on escape

## [0.2.0] - 2021-07-12

- Add support for Job resource - [#42](https://github.com/kdash-rs/kdash/pull/42), Thanks to [somayaj](https://github.com/somayaj)
- Add support for CronJob resource - [#69](https://github.com/kdash-rs/kdash/pull/69), Thanks to [somayaj](https://github.com/somayaj)
- Add support for DaemonSets
- Add support for Secrets
- Add more resources tab and menu
- Show init containers in container view
- Internal optimizations

## [0.1.2] - 2021-06-12

- Add human friendly crash messages
- Add Tab keybinding to cycle through views
- Migrate to kubectl-view-allocations library

## [0.1.1] - 2021-06-04

- Fix a small bug that crashes the app in certain terminal size

## [0.1.0] - 2021-05-17

- Stable release
- Minor bug fixes
- Add vim key bindings for arrow keys
- Chocolatey deployment for Windows 10

## [0.0.9] - 2021-05-10

- Improved error handling and error display
- Minor bug fixes and improvements

## [0.0.8] - 2021-05-04

### Added

- Get YAML for all resources (pod, svc, node, statefulset, replicaset, configmap, deployment)
- Describe for all remaining resources (svc, statefulset, replicaset, configmap, deployment)

### Changed

- Table scrolling doesn't circle back now. This seems to be better UX when having long lists

### Fixed

- Describe view spacing

## [0.0.7] - 2021-05-03

### Added

- Container ports and probes

### Fixed

- Library updates
- Scroll improvements
- More tests
- Show containers for failing pods

## [0.0.6] - 2021-04-27

### Added

- Switch k8s contexts from the all contexts view

## [0.0.5] - 2021-04-27

### Fixed

- Scrolling issues
- Log streaming discrepancy
- CLI versions UI glitch

## [0.0.4] - 2021-04-26

### Added

- Homebrew installation
- Docker installation

## [0.0.3] - 2021-04-25

### Fixed

- Minor bug fixes
- Refactor and polish

### Added

- Resource utilization view with grouping
- Select/copy text in logs and describe view
- Config map tab
- Statefulsets tab
- Replicasets tab
- Deployments tab

## [0.0.2] - 2021-04-22

### Fixed

- Pod status fix
- Switch to API for metrics
- Various bug fixes
- Update key bindings
- Update theme consistency

### Added

- Containers view
- Container logs
- Pod describe
- Node describe

## [0.0.1] - 2021-04-18

- Initial beta release

---

# What is this?

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
