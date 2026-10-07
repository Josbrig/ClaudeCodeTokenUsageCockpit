# Changelog

All notable changes to this project are documented in this file.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed
- On Windows the bridge can be set up from a folder whose path contains a space (short folder name or quoted path); Linux and macOS still need a folder without spaces.
- The Windows x64 executable no longer imports the Visual C++ runtime (the C runtime is linked in; not yet tried on a clean Windows); the bridge starts faster (median 66 ms instead of 101 ms on the development machine).

### Added
- Project brief, project description, research note on data sources and first draft of the requirements.
- Repository setup: issue forms, pull request template, CI check, Dependabot configuration, contribution and security guidelines.
