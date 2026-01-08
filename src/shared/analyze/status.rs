use tracing::{error, info, warn};

#[derive(Copy, Clone, Debug)]
pub enum AnalyzeStatus {
    NoKnownErrorsFound,
    KnownErrorFoundNoFixFound,
    KnownErrorFoundUserDenied,
    KnownErrorFoundFixFailed,
    KnownErrorFoundFixSucceeded,
}

pub fn report_result(status: &AnalyzeStatus) {
    match status {
        AnalyzeStatus::NoKnownErrorsFound => info!(target: "always", "No known errors found"),
        AnalyzeStatus::KnownErrorFoundNoFixFound => {
            info!(target: "always", "No automatic fix available")
        }
        AnalyzeStatus::KnownErrorFoundUserDenied => warn!(target: "always", "User denied fix"),
        AnalyzeStatus::KnownErrorFoundFixFailed => error!(target: "always", "Fix failed"),
        AnalyzeStatus::KnownErrorFoundFixSucceeded => info!(target: "always", "Fix succeeded"),
    }
}

impl AnalyzeStatus {
    pub fn to_exit_code(self) -> i32 {
        match self {
            // we need this to return a success code
            AnalyzeStatus::KnownErrorFoundFixSucceeded => 0,
            // all others can return their discriminant value
            status => status as i32,
        }
    }
}
