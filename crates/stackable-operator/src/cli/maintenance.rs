use clap::Args;

use crate::eos::EndOfSupportOptions;

#[derive(Debug, PartialEq, Eq, Args)]
#[command(next_help_heading = "Maintenance Options")]
pub struct MaintenanceOptions {
    /// Don't maintain the CustomResourceDefinitions (CRDs) the operator is responsible for.
    ///
    /// Maintenance includes creating the CRD initially, adding new versions and keeping the TLS
    /// certificate of webhooks up to date. Turning this off can be desirable to reduce the RBAC
    /// permissions of the operator.
    ///
    /// WARNING: If you disable CRD maintenance you are responsible for maintaining it, including,
    /// but not limited to, the points above.
    #[arg(long, env)]
    pub disable_crd_maintenance: bool,

    // IMPORTANT: All (flattened) sub structs should be placed at the end to ensure the help
    // headings are correct.
    #[command(flatten)]
    pub end_of_support: EndOfSupportOptions,
}
