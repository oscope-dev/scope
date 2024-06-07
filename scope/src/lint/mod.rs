pub mod cli {
    use clap::Args;

    #[derive(Debug, Args)]
    pub struct LintArgs {}
}

pub mod commands {
    use crate::prelude::UnstructuredReportBuilder;
    use crate::prelude::{
        ActionReport, ActionTaskReport, ConfigOptions, DefaultExecutionProvider,
        DefaultGroupedReportBuilder, DefaultUnstructuredReportBuilder, FoundConfig, GroupReport,
        GroupedReportBuilder, LintArgs, OutputCaptureBuilder, ReportRenderer,
    };
    use anyhow::Result;
    use chrono::DateTime;
    use fake::faker::lorem::en::*;
    use std::sync::Arc;
    use fake::Fake;
    use tracing::info;

    pub async fn lint_root(found_config: &FoundConfig, _args: &LintArgs) -> Result<i32> {
        let unstructured = default_unstructured()?;
        let structured = default_structured()?;
        let exec_runner = Arc::new(DefaultExecutionProvider::default());

        for (name, location) in &found_config.report_upload {
            let mut unstructured_builder = unstructured.clone();
            unstructured_builder.run_and_append_additional_data(
                found_config,
                exec_runner.clone(),
                &location.additional_data,
            ).await?;

            let unstructured_report = unstructured_builder.render(&location)?;

            let unstructured_path = format!(
                "{}-unstructured-{}.md",
                name,
                ConfigOptions::generate_run_id()
            );
            info!(target: "always", "Creating template at {}", unstructured_path);

            std::fs::write(unstructured_path,
                format!(
                    "{}\n{}",
                    unstructured_report.title(),
                    unstructured_report.body()
                ),
            )?;

            let mut structured_builder = structured.clone();
            structured_builder.run_and_append_additional_data(
                found_config,
                exec_runner.clone(),
                &location.additional_data,
            ).await?;

            let structured_report = structured_builder.render(&location)?;

            let structured_path = format!(
                "{}-structured-{}.md",
                name,
                ConfigOptions::generate_run_id()
            );

            info!(target: "always", "Creating template at {}", structured_path);

            std::fs::write(structured_path,
                format!(
                    "{}\n{}",
                    structured_report.title(),
                    structured_report.body()
                ),
            )?;
        }

        Ok(0)
    }
    fn default_structured() -> Result<DefaultGroupedReportBuilder> {
        let mut builder = DefaultGroupedReportBuilder::new("sample command");
        builder.append_group(&make_group(1))?;
        builder.append_group(&make_group(2))?;
        builder.append_group(&make_group(3))?;

        Ok(builder)
    }

    fn make_action_report(prefix: &str, idx: i64) -> ActionReport {
        let action_report = || -> ActionTaskReport {
            ActionTaskReport {
                command: Words(1..2).fake::<Vec<String>>().join(" "),
                output: Some((3..7).map(|_| make_line()).collect::<Vec<_>>().join("\n")),
                exit_code: Some(0),
                start_time: DateTime::from_timestamp(1715612600, 0).unwrap(),
                end_time: DateTime::from_timestamp(1715612699, 0).unwrap(),
            }
        };

        ActionReport {
            action_name: format!("{} {}", prefix, idx),
            check: vec![action_report()],
            fix: vec![action_report()],
            validate: vec![action_report(), action_report()],
        }
    }

    fn make_group(offset: i64) -> GroupReport {
        let group_name = format!("group {}", offset);
        let mut group = GroupReport::new(&group_name);
        group.add_action(&make_action_report(&group_name, offset * 10 + 1));
        group.add_action(&make_action_report(&group_name, offset * 10 + 2));

        group
    }

    fn default_unstructured() -> Result<DefaultUnstructuredReportBuilder> {
        let mut std_out = Vec::new();
        let mut std_err = Vec::new();
        let start_time = 1715612600;
        for idx in 0..15 {
            if idx % 3 == 0 {
                std_err.push((
                    DateTime::from_timestamp(start_time + idx * 10, 0).unwrap(),
                    make_line(),
                ));
            } else {
                std_out.push((
                    DateTime::from_timestamp(start_time + idx * 10, 0).unwrap(),
                    make_line(),
                ));
            }
        }

        let capture = OutputCaptureBuilder::default()
            .command("sample command")
            .stdout(std_out)
            .stderr(std_err)
            .exit_code(1)
            .start_time(DateTime::from_timestamp(1715612600, 0).unwrap())
            .end_time(DateTime::from_timestamp(1715612600 + 20 * 10, 0).unwrap())
            .build()?;

        Ok(DefaultUnstructuredReportBuilder::new("sample command", &capture))
    }

    fn make_line() -> String {
        Words(7..30).fake::<Vec<String>>().join(" ")
    }
}

pub mod prelude {
    pub use super::{cli::LintArgs, commands::lint_root};
}
