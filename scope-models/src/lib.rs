use crate::core::{ModelMetadata, ModelRoot};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;

mod core;
mod v1alpha;

pub mod prelude {
    pub use crate::core::*;
    pub use crate::v1alpha::prelude::*;
    pub use crate::ScopeModel;
}

pub trait ScopeModel<S> {
    fn api_version(&self) -> String;
    fn kind(&self) -> String;
    fn metadata(&self) -> &ModelMetadata;
    fn spec(&self) -> &S;
    fn name(&self) -> &str {
        &self.metadata().name
    }
    fn file_path(&self) -> String {
        self.metadata().file_path()
    }
    fn containing_dir(&self) -> String {
        self.metadata().containing_dir()
    }

    fn exec_path(&self) -> String {
        self.metadata().exec_path()
    }
    fn full_name(&self) -> String {
        format!("{}/{}", self.kind(), self.name())
    }
}

pub trait InternalScopeModel<S, R>:
    JsonSchema + Serialize + for<'a> Deserialize<'a> + ScopeModel<S>
where
    R: for<'a> Deserialize<'a>,
{
    fn int_api_version() -> String;
    fn int_kind() -> String;
    fn known_type(input: &ModelRoot<Value>) -> anyhow::Result<Option<R>> {
        if Self::int_api_version().to_lowercase() == input.api_version.to_lowercase()
            && Self::int_kind().to_lowercase() == input.kind.to_lowercase()
        {
            let value = serde_json::to_value(input)?;
            return Ok(Some(serde_json::from_value::<R>(value)?));
        }
        Ok(None)
    }

    #[cfg(test)]
    fn examples() -> Vec<String>;

    #[cfg(test)]
    fn create_and_validate(
        schema_gen: &mut schemars::gen::SchemaGenerator,
        out_dir: &str,
        merged_schema: &str,
    ) -> anyhow::Result<()> {
        let schema = schema_gen.root_schema_for::<Self>();
        let schema_json = serde_json::to_string_pretty(&schema)?;

        let path_prefix: String = Self::int_api_version()
            .split(&['.', '/'])
            .rev()
            .collect::<Vec<_>>()
            .join(".");

        std::fs::write(
            format!("{}/{}.{}.json", out_dir, path_prefix, Self::int_kind()),
            &schema_json,
        )?;

        for example in Self::examples() {
            validate_schema::<Self>(&schema_json, &example)?;
            validate_schema::<Self>(merged_schema, &example)?;
        }
        Ok(())
    }
}

#[cfg(test)]
pub fn make_schema_generator() -> schemars::gen::SchemaGenerator {
    let settings = schemars::gen::SchemaSettings::draft2019_09().with(|s| {
        s.option_nullable = true;
    });
    settings.into_generator()
}

#[cfg(test)]
fn validate_schema<T>(schema_json: &str, example_path: &str) -> anyhow::Result<()>
where
    T: schemars::JsonSchema + for<'a> serde::Deserialize<'a> + Serialize,
{
    let example = std::fs::read_to_string(format!(
        "{}/examples/{}",
        env!("CARGO_MANIFEST_DIR"),
        example_path
    ))
    .unwrap();
    let parsed: T = serde_yaml::from_str(&example)?;

    let schema = serde_json::from_str(schema_json)?;

    let compiled_schema = jsonschema::JSONSchema::compile(&schema).expect("A valid schema");

    let parsed_json = serde_json::to_value(&parsed)?;
    if let Err(err_iter) = compiled_schema.validate(&parsed_json) {
        println!("{}", serde_json::to_string_pretty(&parsed_json).unwrap());
        for e in err_iter {
            println!("error: {}", e);
        }
        unreachable!();
    };

    Ok(())
}

#[cfg(test)]
mod schema_gen {
    use crate::v1alpha::prelude::*;
    use crate::InternalScopeModel;

    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
    #[serde(untagged)]
    enum ScopeTypes {
        ReportLocation(V1AlphaReportLocation),
        ReportDefinition(V1AlphaReportDefinition),
        KnownError(V1AlphaKnownError),
        DoctorGroup(V1AlphaDoctorGroup),
    }

    #[test]
    fn create_and_validate_schemas() {
        let out_dir = format!("{}/schema", env!("CARGO_MANIFEST_DIR"));
        std::fs::remove_dir_all(&out_dir).unwrap();
        std::fs::create_dir_all(&out_dir).unwrap();

        let mut schema_gen = crate::make_schema_generator();
        let merged_schema = schema_gen.root_schema_for::<ScopeTypes>();
        let merged_schema_json = serde_json::to_string_pretty(&merged_schema).unwrap();
        std::fs::write(format!("{}/merged.json", out_dir), &merged_schema_json).unwrap();

        V1AlphaReportLocation::create_and_validate(&mut schema_gen, &out_dir, &merged_schema_json)
            .unwrap();
        V1AlphaReportDefinition::create_and_validate(
            &mut schema_gen,
            &out_dir,
            &merged_schema_json,
        )
        .unwrap();
        V1AlphaKnownError::create_and_validate(&mut schema_gen, &out_dir, &merged_schema_json)
            .unwrap();
        V1AlphaDoctorGroup::create_and_validate(&mut schema_gen, &out_dir, &merged_schema_json)
            .unwrap();
    }
}
