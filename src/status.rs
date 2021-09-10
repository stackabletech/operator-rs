//! This module provides structs and trades to generalize the custom resource status access.
use crate::command::CommandRef;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Condition;

pub trait Conditions {
    fn conditions(&self) -> Option<&[Condition]>;
    fn conditions_mut(&mut self) -> &mut Vec<Condition>;
}

pub trait HasCurrentCommand {
    fn current_command(&self) -> Option<CommandRef>;

    // TODO: setters are non-rusty, is there a better way? Dirkjan?
    fn set_current_command(&mut self, command: CommandRef);

    fn tracking_location() -> &'static str;

    fn clear_current_command(&mut self);
}
