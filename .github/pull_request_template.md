# Description

*Please add a description here. This will become the commit message of the merge request later.*

## Definition of Done Checklist

- Not all of these items are applicable to all PRs, the author should update this template to only leave the boxes in that are relevant
- Please make sure all these things are done and tick the boxes

### Author

- [ ] Changes are OpenShift compatible
- [ ] CRD changes approved
- [ ] CRD documentation for all fields, following the [style guide](https://docs.stackable.tech/home/nightly/contributor/docs/style-guide).
- [ ] Integration tests passed (for non trivial changes)
- [ ] Changes need to be "offline" compatible

### Reviewer

- [ ] Code contains useful comments
- [ ] Code contains useful logging statements
- [ ] (Integration-)Test cases added
- [ ] Documentation added or updated. Follows the [style guide](https://docs.stackable.tech/home/nightly/contributor/docs/style-guide).
- [ ] Changelog updated
- [ ] Cargo.toml only contains references to git tags (not specific commits or branches)

### Acceptance

- [ ] Feature Tracker has been updated
- [ ] Proper release label has been added
