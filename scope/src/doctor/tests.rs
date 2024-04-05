use std::collections::BTreeMap;

use crate::models::prelude::{ModelMetadataAnnotations, ModelMetadataBuilder};
use crate::shared::prelude::{DoctorGroup, DoctorGroupAction, DoctorGroupBuilder};

pub fn make_root_model_additional<Meta, Group>(
    actions: Vec<DoctorGroupAction>,
    edit_meta: Meta,
    edit_group: Group,
) -> DoctorGroup
where
    Meta: FnOnce(&mut ModelMetadataBuilder) -> &mut ModelMetadataBuilder,
    Group: FnOnce(&mut DoctorGroupBuilder) -> &mut DoctorGroupBuilder,
{
    let mut binding = ModelMetadataBuilder::default();
    let metadata_builder = binding
        .name("fake-name")
        .description("a description")
        .annotations(ModelMetadataAnnotations::default())
        .labels(BTreeMap::default());
    let metadata = edit_meta(metadata_builder).build().unwrap();

    let mut binding = DoctorGroupBuilder::default();
    let group_builder = binding
        .full_name("DoctorGroup/fake-name")
        .actions(actions)
        .metadata(metadata)
        .requires(Vec::new())
        .run_by_default(true);

    edit_group(group_builder).build().unwrap()
}

pub fn meta_noop(input: &mut ModelMetadataBuilder) -> &mut ModelMetadataBuilder {
    input
}

pub fn group_noop(input: &mut DoctorGroupBuilder) -> &mut DoctorGroupBuilder {
    input
}

pub fn build_root_model(actions: Vec<DoctorGroupAction>) -> DoctorGroup {
    make_root_model_additional(actions, meta_noop, group_noop)
}
