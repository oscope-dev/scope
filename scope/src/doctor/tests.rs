use crate::shared::prelude::{
    DoctorGroup, DoctorGroupAction, DoctorGroupBuilder, ModelMetadataBuilder, ModelRoot,
    ModelRootBuilder,
};
use std::collections::BTreeMap;

pub fn make_root_model_additional<Meta, Root, Group>(
    actions: Vec<DoctorGroupAction>,
    edit_meta: Meta,
    edit_root: Root,
    edit_group: Group,
) -> ModelRoot<DoctorGroup>
where
    Meta: FnOnce(&mut ModelMetadataBuilder) -> &mut ModelMetadataBuilder,
    Root: FnOnce(&mut ModelRootBuilder<DoctorGroup>) -> &mut ModelRootBuilder<DoctorGroup>,
    Group: FnOnce(&mut DoctorGroupBuilder) -> &mut DoctorGroupBuilder,
{
    let mut binding = DoctorGroupBuilder::default();
    let group_builder = binding
        .description("a description")
        .actions(actions)
        .requires(Vec::new());

    let group = edit_group(group_builder).build().unwrap();

    let mut binding = ModelMetadataBuilder::default();
    let metadata_builder = binding
        .name("fake-name")
        .annotations(BTreeMap::default())
        .labels(BTreeMap::default());
    let metadata = edit_meta(metadata_builder).build().unwrap();

    let mut binding = ModelRootBuilder::default();
    let root_builder = binding
        .api_version("fake")
        .kind("fake-kind")
        .metadata(metadata)
        .spec(group);

    edit_root(root_builder).build().unwrap()
}

pub fn meta_noop(input: &mut ModelMetadataBuilder) -> &mut ModelMetadataBuilder {
    input
}

pub fn root_noop(input: &mut ModelRootBuilder<DoctorGroup>) -> &mut ModelRootBuilder<DoctorGroup> {
    input
}

pub fn group_noop(input: &mut DoctorGroupBuilder) -> &mut DoctorGroupBuilder {
    input
}

pub fn build_root_model(actions: Vec<DoctorGroupAction>) -> ModelRoot<DoctorGroup> {
    make_root_model_additional(actions, meta_noop, root_noop, group_noop)
}
