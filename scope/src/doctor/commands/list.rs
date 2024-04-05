use std::collections::BTreeSet;

use anyhow::Result;
use clap::Args;

use crate::doctor::runner::compute_group_order;
use crate::report_stdout;
use crate::shared::prelude::{DoctorGroup, FoundConfig};
use crate::shared::print_details;

#[derive(Debug, Args)]
pub struct DoctorListArgs {}

pub async fn doctor_list(found_config: &FoundConfig, _args: &DoctorListArgs) -> Result<()> {
    report_stdout!("Available checks that will run");
    let order = generate_doctor_list(found_config).clone();
    print_details(&found_config.working_dir, &order).await;
    Ok(())
}

pub fn generate_doctor_list(found_config: &FoundConfig) -> Vec<DoctorGroup> {
    let all_keys = BTreeSet::from_iter(
        found_config
            .doctor_group
            .iter()
            .filter(|(_, v)| v.run_by_default)
            .map(|(k, _)| k.to_string()),
    );
    let group_order = compute_group_order(&found_config.doctor_group, all_keys);

    group_order
        .iter()
        .map(|name| found_config.doctor_group.get(name).unwrap().clone())
        .collect()
}
