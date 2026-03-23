pub mod crafting_domain;
pub mod crafting_solver;
pub mod demo_scenario;
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