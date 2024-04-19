# k8s-version

A small helper crate to parse and validate Kubernetes resource API versions.

```rust
use k8s_version::ApiVersion;

let api_version = ApiVersion::from_str("extensions/v1beta1")?;
```
