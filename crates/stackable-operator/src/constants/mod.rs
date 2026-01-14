use std::sync::LazyLock;

use crate::kvp::Label;

pub mod secret;

pub static RESTART_CONTROLLER_ENABLED_LABEL: LazyLock<Label> = LazyLock::new(|| {
    Label::try_from(("restarter.stackable.tech/enabled", "true"))
        .expect("static label is always valid")
});
