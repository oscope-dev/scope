use crate::doctor::runner::compute_group_order_2;
use crate::shared::prelude::{DoctorGroup, FoundConfig};
use crate::shared::print_details;
use anyhow::Result;
use clap::Args;
use std::collections::{BTreeSet, VecDeque};
use tracing::info;

#[derive(Debug, Args)]
pub struct DoctorListArgs {}

pub async fn doctor_list(found_config: &FoundConfig, _args: &DoctorListArgs) -> Result<()> {
    info!(target: "user", "Available checks that will run");
    let order = generate_doctor_list(found_config).clone();
    print_details(&found_config.working_dir, &order);
    Ok(())
}

pub fn generate_doctor_list(found_config: &FoundConfig) -> Vec<DoctorGroup> {
    let all_keys = BTreeSet::from_iter(found_config.doctor_group.keys().map(|x| x.to_string()));
    let group_order = compute_group_order_2(&found_config.doctor_group, all_keys);

    group_order
        .iter()
        .map(|name| found_config.doctor_group.get(name).unwrap().clone())
        .collect()
}
