use k8s_openapi::api::core::v1::CSIVolumeSource;
use std::collections::BTreeMap;

#[derive(Clone)]
pub struct SecretOperatorVolumeSourceBuilder {
    secret_class: String,
    scopes: Vec<SecretOperatorVolumeScope>,
}

impl SecretOperatorVolumeSourceBuilder {
    pub fn new(secret_class: impl Into<String>) -> Self {
        Self {
            secret_class: secret_class.into(),
            scopes: Vec::new(),
        }
    }

    pub fn with_node_scope(&mut self) -> &mut Self {
        self.scopes.push(SecretOperatorVolumeScope::Node);
        self
    }

    pub fn with_pod_scope(&mut self) -> &mut Self {
        self.scopes.push(SecretOperatorVolumeScope::Pod);
        self
    }

    pub fn with_service_scope(&mut self, name: impl Into<String>) -> &mut Self {
        self.scopes
            .push(SecretOperatorVolumeScope::Service { name: name.into() });
        self
    }

    pub fn build(&self) -> CSIVolumeSource {
        let mut attrs = BTreeMap::from([(
            "secrets.stackable.tech/class".to_string(),
            self.secret_class.clone(),
        )]);

        if !self.scopes.is_empty() {
            let mut scopes = String::new();
            for scope in self.scopes.iter() {
                if !scopes.is_empty() {
                    scopes.push(',');
                };
                match scope {
                    SecretOperatorVolumeScope::Node => scopes.push_str("node"),
                    SecretOperatorVolumeScope::Pod => scopes.push_str("pod"),
                    SecretOperatorVolumeScope::Service { name } => {
                        scopes.push_str("service=");
                        scopes.push_str(name);
                    }
                }
            }
            attrs.insert("secrets.stackable.tech/scope".to_string(), scopes);
        }

        CSIVolumeSource {
            driver: "secrets.stackable.tech".to_string(),
            volume_attributes: Some(attrs),
            ..CSIVolumeSource::default()
        }
    }
}

#[derive(Clone)]
enum SecretOperatorVolumeScope {
    Node,
    Pod,
    Service { name: String },
}

#[cfg(test)]
mod tests {}
