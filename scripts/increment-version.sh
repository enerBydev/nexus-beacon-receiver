#!/bin/sh

# increment-version.sh - Script to increment version numbers
# Usage: increment-version.sh [patch|minor|major]

if [ "$#" -ne 1 ]; then
    echo "Usage: $0 [patch|minor|major]"
    exit 1
fi

BUMP_TYPE="$1"
CURRENT_VERSION=$(cat VERSION 2>/dev/null || echo "0.1.0")

# Parse current version using sed instead of arrays
MAJOR=$(echo "$CURRENT_VERSION" | sed 's/\..*//')
REMAINDER=$(echo "$CURRENT_VERSION" | sed 's/[^.]*\.//')
MINOR=$(echo "$REMAINDER" | sed 's/\..*//')
PATCH=$(echo "$REMAINDER" | sed 's/[^.]*\.//')

case "$BUMP_TYPE" in
    major)
        NEW_MAJOR=$((MAJOR + 1))
        echo "$NEW_MAJOR.0.0"
        ;;
    minor)
        NEW_MINOR=$((MINOR + 1))
        echo "$MAJOR.$NEW_MINOR.0"
        ;;
    patch)
        NEW_PATCH=$((PATCH + 1))
        echo "$MAJOR.$MINOR.$NEW_PATCH"
        ;;
    *)
        echo "Invalid bump type: $BUMP_TYPE"
        exit 1
        ;;
esac