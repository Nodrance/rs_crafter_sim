// This just exports everything so it can be tested.
// Also defines whether debug logging is enabled and provides a helper macro for it.

pub mod model;
pub mod crafting_solver;
pub mod demo_scenarios;
pub mod execution_planner;
pub mod progress_logger;
pub mod recipe_analysis;

pub const DEBUG_LOGGING_ENABLED: bool = true;

#[macro_export]
macro_rules! debugln {
	($($arg:tt)*) => {{
		if $crate::DEBUG_LOGGING_ENABLED {
			println!($($arg)*);
		}
	}};
}