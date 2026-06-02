# Branch Protection Rules

## Main Branch Protection

The `main` branch is protected with the following rules:

### Required Pull Request Reviews
- Require pull request reviews before merging
- Require at least 1 approval from CODEOWNERS
- Dismiss stale pull request approvals when new commits are added
- Require status checks to pass before merging:
  - test
  - lint
  - format
  - build-wasm

### Required Status Checks
- CI Pipeline must pass
- All GitHub Actions must pass

### Required Checks
- All commits must be linked to GitHub users
- Non-fast-forward merges are not allowed
- Linear history required

## Version Management
Version is managed through:
- VERSION file containing the current version string
- Cargo.toml containing the package version
- CHANGELOG.md containing release notes

Version numbers follow semantic versioning (semver):
- MAJOR.MINOR.PATCH format
- Features and enhancements increment MINOR
- Bug fixes increment PATCH
- Breaking changes increment MAJOR

## Protected Branch Rules
- No force push to main branch
- No deletion of main branch
- Pushes to main must go through pull requests
- All changes must be reviewed through pull requests
- All commits must follow conventional commit format