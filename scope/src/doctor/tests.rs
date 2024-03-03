use crate::shared::prelude::{
    DoctorGroup, DoctorGroupAction, DoctorGroupBuilder,
};
use std::collections::BTreeMap;
use dev_scope_model::prelude::{ModelMetadataBuilder, ModelRootBuilder};

pub fn make_root_model_additional<Meta, Root, Group>(
    actions: Vec<DoctorGroupAction>,
    edit_meta: Meta,
    edit_group: Group,
) -> DoctorGroup
where
    Meta: FnOnce(&mut ModelMetadataBuilder) -> &mut ModelMetadataBuilder,
    Root: FnOnce(&mut ModelRootBuilder<DoctorGroup>) -> &mut ModelRootBuilder<DoctorGroup>,
    Group: FnOnce(&mut DoctorGroupBuilder) -> &mut DoctorGroupBuilder,
{
    let mut binding = ModelMetadataBuilder::default();
    let metadata_builder = binding
        .name("fake-name")
        .annotations(BTreeMap::default())
        .labels(BTreeMap::default());
    let metadata = edit_meta(metadata_builder).build().unwrap();

    let mut binding = DoctorGroupBuilder::default();
    let group_builder = binding
        .description("a description")
        .actions(actions)
        .metadata(metadata)
        .requires(Vec::new());

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
