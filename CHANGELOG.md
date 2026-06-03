# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

### Changed

### Fixed

---

## [0.3.0] - 2026-06-03

### Added

### Changed

### Fixed

---

## [0.2.2] - 2026-06-03

### Added

### Changed

### Fixed

---

## [0.2.1] - 2026-06-03

### Added

### Changed

### Fixed

---

## [0.2.0] - 2026-06-03

### Added

### Changed

### Fixed

---

## [0.1.1] - 2026-06-02

### Added

### Changed

### Fixed

---

## [0.1.0] - 2026-06-02

### Added
- Initial release of NEXUS Beacon Receiver
- Cloudflare Worker (Rust/WASM) receiving daily telemetry beacons
- D1 database schema for beacons and daily global stats
- POST /v1/beacon endpoint (stub)
- GET /v1/stats endpoint (stub)
- GET /v1/stats/summary endpoint (stub)
- CI/CD pipeline replicated from NEXUS-AI-Gateway
- Git hooks for conventional commits, format checking, linting
- Taskfile.yaml for build, test, deploy workflow