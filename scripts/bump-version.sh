#!/bin/sh

# bump-version.sh - Script to bump version and update files
# Usage: bump-version.sh [patch|minor|major]

set -e

BUMP_TYPE="${1:?Usage: $0 [patch|minor|major]}"

# Get next version
NEXT_VERSION=$(scripts/increment-version.sh "$BUMP_TYPE") || exit 1
CURRENT_VERSION=$(cat VERSION)

echo "Bumping version: $CURRENT_VERSION → $NEXT_VERSION ($BUMP_TYPE)"

# Update VERSION file
echo "$NEXT_VERSION" > VERSION

# Update Cargo.toml
sed -i "s/^version = \".*\"/version = \"$NEXT_VERSION\"/" Cargo.toml

# Commit and tag
git add VERSION Cargo.toml
git commit -m "chore: bump version to $NEXT_VERSION"
git tag -a "v$NEXT_VERSION" -m "Release v$NEXT_VERSION"

echo "Version bumped to $NEXT_VERSION and tagged"
