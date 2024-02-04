pub mod analyze;
pub mod doctor;
pub mod report;
pub mod shared;

pub mod prelude {
    pub use crate::analyze::prelude::*;
    pub use crate::doctor::prelude::*;
    pub use crate::report::prelude::*;
    pub use crate::shared::prelude::*;
}