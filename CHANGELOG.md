# Changelog

## [0.2.1] - 20-July-2021

- Add support for page up and page down on tables and text views
- Fix crash on escape

## [0.2.0] - 12-July-2021

- Add support for Job resource - [#42](https://github.com/kdash-rs/kdash/pull/42), Thanks to [somayaj](https://github.com/somayaj)
- Add support for CronJob resource - [#69](https://github.com/kdash-rs/kdash/pull/69), Thanks to [somayaj](https://github.com/somayaj)
- Add support for DaemonSets
- Add support for Secrets
- Add more resources tab and menu
- Show init containers in container view
- Internal optimizations

## [0.1.2] - 12-June-2021

- Add human friendly crash messages
- Add Tab keybinding to cycle through views
- Migrate to kubectl-view-allocations library

## [0.1.1] - 04-June-2021

- Fix a small bug that crashes the app in certain terminal size

## [0.1.0] - 17-May-2021

- Stable release
- Minor bug fixes
- Add vim key bindings for arrow keys
- Chocolatey deployment for Windows 10

## [0.0.9] - 10-May-2021

- Improved error handling and error display
- Minor bug fixes and improvements

## [0.0.8] - 04-May-2021

### Added

- Get YAML for all resources (pod, svc, node, statefulset, replicaset, configmap, deployment)
- Describe for all remaining resources (svc, statefulset, replicaset, configmap, deployment)

### Changed

- Table scrolling doesn't circle back now. This seems to be better UX when having long lists

### Fixed

- Describe view spacing

## [0.0.7] - 03-May-2021

### Added

- Container ports and probes

### Fixed

- Library updates
- Scroll improvements
- More tests
- Show containers for failing pods

## [0.0.6] - 27-Apr-2021

### Added

- Switch k8s contexts from the all contexts view

## [0.0.5] - 27-Apr-2021

### Fixed

- Scrolling issues
- Log streaming discrepancy
- CLI versions UI glitch

## [0.0.4] - 26-Apr-2021

### Added

- Homebrew installation
- Docker installation

## [0.0.3] - 25-Apr-2021

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

## [0.0.2] - 22-Apr-2021

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

## [0.0.1] - 18-Apr-2021

- Initial beta release

---

# What is this?

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
