# stackable-crd-previewer

The purpose of this crate is to preview the effects of code changes on our CRDs.

Use the following command to re-generate the CRDs:

```bash
cargo check -p stackable-crd-previewer
```

This should implicitly happen by `rust-analyzer` or `cargo check`, so shouldn't be needed to invoke
explicitly normally.

With an existing Kubernetes context you can run the following command to check if the CRDs are valid:

```bash
kubectl apply -f generated-crd-previews/ --dry-run=server
```
