#!/bin/sh

# Auto-version script for nexus-beacon-receiver
# Analyze commits since last tag and determine version bump

set -e

DRY_RUN=false
APPLY=false

# Parse arguments
while [ $# -gt 0 ]]; do
    case $1 in
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        --apply)
            APPLY=true
            shift
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Get the current version from VERSION file
CURRENT_VERSION=$(cat VERSION 2>/dev/null || echo "0.1.0")

# Get the last tag
LAST_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "v$CURRENT_VERSION")

# Get commits since last tag
COMMITS=$(git log "$LAST_TAG"..HEAD --oneline 2>/dev/null || echo "")

# If no commits, exit early
if [ -z "$COMMITS" ]; then
    echo "No commits since last tag $LAST_TAG"
    exit 0
fi

# Determine bump type based on commit types
BUMP_TYPE="none"

# Check for breaking changes or features first
if echo "$COMMITS" | grep -E "^(feat|fix|refactor|perf)!:" > /dev/null; then
    BUMP_TYPE="minor"
elif echo "$COMMITS" | grep -E "^(feat):" > /dev/null; then
    BUMP_TYPE="minor"
elif echo "$COMMITS" | grep -E "^(fix|refactor|perf):" > /dev/null; then
    BUMP_TYPE="patch"
fi

# If no relevant commits, exit
if [ "$BUMP_TYPE" = "none" ]; then
    echo "No version bump needed"
    exit 0
fi

# Calculate next version
NEXT_VERSION=$(scripts/increment-version.sh "$BUMP_TYPE")

if [ "$DRY_RUN" = true ]; then
    echo "Would bump version from $CURRENT_VERSION to $NEXT_VERSION ($BUMP_TYPE)"
    exit 0
fi

if [ "$APPLY" = true ]; then
    echo "Bumping version from $CURRENT_VERSION to $NEXT_VERSION ($BUMP_TYPE)"

    # Update VERSION file
    echo "$NEXT_VERSION" > VERSION

    # Update Cargo.toml
    sed -i "s/^version = \".*\"/version = \"$NEXT_VERSION\"/" Cargo.toml

    # Update CHANGELOG.md with new version entry
    # Create a temporary file with the new entry
    TEMP_CHANGELOG=$(mktemp)
    {
        echo "# Changelog"
        echo ""
        echo "All notable changes to this project will be documented in this file."
        echo ""
        echo "The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),"
        echo "and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html)."
        echo ""
        echo "## [Unreleased]"
        echo ""
        echo "### Added"
        echo ""
        echo "### Changed"
        echo ""
        echo "### Fixed"
        echo ""
        echo "---"
        echo ""
        echo "## [$NEXT_VERSION] - $(date +%Y-%m-%d)"
        echo ""
        echo "### Added"
        echo "- Auto-version bump from $CURRENT_VERSION"
        echo ""
        echo "### Changed"
        echo ""
        echo "### Fixed"
        echo ""
        echo "---"
        tail -n +17 CHANGELOG.md | sed "1d"
    } > "$TEMP_CHANGELOG"

    # Replace the original CHANGELOG.md
    mv "$TEMP_CHANGELOG" CHANGELOG.md

    # Commit the version bump
    git add VERSION Cargo.toml CHANGELOG.md
    git commit -m "chore: bump version to $NEXT_VERSION"

    # Create tag
    git tag "v$NEXT_VERSION"

    echo "Version bumped to $NEXT_VERSION and tagged"
fi