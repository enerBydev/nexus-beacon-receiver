#!/bin/sh
# Setup git hooks for nexus-beacon-receiver
# Uses core.hooksPath to point directly to scripts/hooks/ (NEXUS pattern)

echo "Setting up git hooks..."

# Make all hook scripts executable
chmod +x scripts/hooks/pre-commit
chmod +x scripts/hooks/commit-msg
chmod +x scripts/hooks/pre-push
chmod +x scripts/hooks/post-commit

# Set git to use scripts/hooks/ directly (no copy to .git/hooks/)
git config core.hooksPath scripts/hooks

echo "Git hooks configured via core.hooksPath"
echo ""
echo "Active hooks:"
ls -la scripts/hooks/
