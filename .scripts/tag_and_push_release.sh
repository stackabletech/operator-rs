#!/usr/bin/env bash

# Tag the latest release based on the versions in the Cargo.toml files.
# Do this after merging a release PR to main.
#
# This script will skip local tags (eg: if a crate release has already been
# done). You will be prompted at the end whether to push the tags or not.
#
# To ensure crate versions match the latest version in the changelogs,
# see verify_crate_versions.sh

set -euo pipefail

# You need fzf to run this
type fzf &> /dev/null || (echo "This script requres fzf" >&2 && exit 1)

# Update from remote
git fetch origin

# Select a commit (usually the first is the right one)
SHORT_COMMIT_HASH=$(git log --color=always --oneline | fzf --ansi --layout=reverse --prompt="Select the commit to tag" | cut -d' ' -f1)

# Get the latest versions
LATEST_TAGS=$(find . -mindepth 2 -name CHANGELOG.md -exec sh -c 'grep -HoE "\[[0-9]+\.[0-9]+\.[0-9]+\]" "$1" | head -1 | sed -e "s|^./crates/\([a-z0-9_\-]\+\).*:\[\(.*\)\]$|\1-\2|"' shell {} \; | sort)
LATEST_TAGS=$(find . -mindepth 2 -name CHANGELOG.md -exec sh -c 'grep -HoE "\[[0-9]+\.[0-9]+\.[0-9]+\]" "$1" | head -1 | sed -e "s|^./crates/\([a-z0-9_\-]\+\).*:\[\(.*\)\]$|\1-\2|"' shell {} \; | sort)
ALL_TAGS=$(git tag | sort)

# Only show tags that don't exist (if you need to overwrite tags, do it manually)
AVAILABLE_TAGS=$(comm -23 <(echo -e "$LATEST_TAGS") <(echo -e "$ALL_TAGS"))

# Select tags
SELECTED_TAGS=$(echo -e "$AVAILABLE_TAGS" | fzf --reverse --multi --bind "ctrl-a:toggle-all" --bind "space:toggle" --prompt "Select multiple to release (use SPACE to select, CTRL+A to invert selection)")

# Tag each selected to the selected commit
echo -e "$SELECTED_TAGS" | xargs -I {} git tag -s -m "release/{}" {} "$SHORT_COMMIT_HASH"
echo
read -r -p "push the tags [y/N]? " PUSH
if [ "${PUSH,,}" == "y" ]; then
    echo -e "$SELECTED_TAGS" | xargs -I {} git push origin {}
fi

git log -1 --oneline
