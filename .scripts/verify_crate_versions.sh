#!/usr/bin/env bash

set -euo pipefail

# Eg: makes a list like this based on the latest release in the CHANGELOG.md files:
# k8s-version-0.1.1
# stackable-certs-0.3.1
# stackable-operator-0.70.0
# stackable-operator-derive-0.3.1
# stackable-telemetry-0.2.0
# stackable-versioned-0.1.1
# stackable-webhook-0.3.1

for CRATE in $(find ./crates/ -mindepth 2 -name Cargo.toml -print0 | xargs -0 -n 1 dirname | xargs -n 1 basename | sort); do
    # Get the version in Cargo.toml
    CRATE_VERSION=$(grep 'version' "./crates/$CRATE/Cargo.toml" | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1)
    [ -n "$CRATE_VERSION" ] || (
        echo "CRATE_VERSION for $CRATE is empty." >&2
        echo "Please check ./crates/$CRATE/Cargo.toml" >&2
        exit 21
    )

    # Special treatment of stackable-versioned-macros:
    # - It has no changelog
    # - The version should be the same as stackable-versioned
    if [ "$CRATE" = "stackable-versioned-macros" ]; then
        ASSOCIATED_CRATE="stackable-versioned"

        # Get the version in Cargo.toml
        ASSOCIATED_CRATE_VERSION=$(grep 'version' "./crates/$ASSOCIATED_CRATE/Cargo.toml" | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1)
        [ -n "$ASSOCIATED_CRATE_VERSION" ] || (
            echo "ASSOCIATED_CRATE_VERSION for $ASSOCIATED_CRATE_VERSION is empty" >&2
            echo "Please check ./crates/$ASSOCIATED_CRATE_VERSION/Cargo.toml" >&2
            exit 22
        )

        # Ensure the versions match
        [ "$CRATE_VERSION" == "$ASSOCIATED_CRATE_VERSION" ] || (
            echo "Versions for $CRATE and $ASSOCIATED_CRATE do not match. $CHANGELOG_VERSION != $ASSOCIATED_CRATE_VERSION" >&2
            echo "Ensure the version in ./crates/$CRATE/Cargo.toml matches the version in ./crates/$ASSOCIATED_CRATE/Cargo.toml" >&2
            exit 23
        )

    else
        # Get the latest documented version from the CHANGELOG.md
        CHANGELOG_VERSION=$(grep -oE '\[[0-9]+\.[0-9]+\.[0-9]+\]' "./crates/$CRATE/CHANGELOG.md" | head -1 | tr -d '[]')
        [ -n "$CHANGELOG_VERSION" ] || (
            echo "CHANGELOG_VERSION for $CRATE is empty" >&2
            echo "Please check the latest release version in ./crates/$CRATE/CHANGELOG.md" >&2
            exit 24
        )

        # Ensure the versions match
        [ "$CRATE_VERSION" == "$CHANGELOG_VERSION" ] || (
            echo "Versions for $CRATE do not match. $CHANGELOG_VERSION != $CRATE_VERSION." >&2
            echo "Ensure the version in ./crates/$CRATE/CHANGELOG.md matches the latest release version in ./crates/$CRATE/Cargo.toml" >&2
            exit 25
        )
    fi

    echo "${CRATE}-${CRATE_VERSION}"

done
