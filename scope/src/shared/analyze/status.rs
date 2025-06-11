#[derive(Copy, Clone)]
pub enum AnalyzeStatus {
    NoKnownErrorsFound,
    KnownErrorFoundNoFixFound,
    KnownErrorFoundUserDenied,
    KnownErrorFoundFixFailed,
    KnownErrorFoundFixSucceeded,
}
