# Contributing

Thank you for your interest. The project is in its concept phase; the way work is organised is described below.

## Issues

- Use one of the issue forms. Blank issues are disabled.
- One issue describes one thing that can be closed on its own.
- Requirements live in [docs/requirements.md](docs/requirements.md). Proposals to change them are welcome as issues; the project owner decides.

## Branches, commits and pull requests

- Branch names: `feature/<issue>-<slug>`, `fix/<issue>-<slug>`, `docs/<issue>-<slug>`.
- Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/) and reference the issue in the footer: `Refs #<issue>`.
- Every change goes through a pull request using the template. The required CI check must pass.
- Pull requests are merged with a merge commit; branches are kept.

## AI-assisted contributions

AI tools may be used. Disclose their use in the pull request description. You are responsible for every line you submit: you must understand it and be able to explain it.

## Changelog

Add an entry under `Unreleased` in [CHANGELOG.md](CHANGELOG.md) for every user-visible change.

## Licence

By contributing you agree that your contribution is licensed under the [Apache License 2.0](LICENSE).
