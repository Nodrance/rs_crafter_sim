# How to use
0. Install cargo and rust from https://www.rust-lang.org/tools/install
1. Edit `get_recipes()`, `get_starting_items()`, and `get_target()` in `src/demo_scenario.rs` to define your crafting scenario. (line 315 and below)
2. Run `cargo run` in your terminal

# Solver test harness
1. Edit JSON files in `tests/cases/` to define sequential solver test cases.
2. Run `cargo test --test solver_harness`.
3. Harness files are loaded in sorted filename order, and cases run sequentially.
4. Each case should include a `description` explaining the inputs, expected outputs, and reasoning.
5. Each case must set `expect.status` to either:
	- `ok`: and provide `remaining_inventory` and `recipe_invocation_counts`
	- `error`: to assert that the solver returns an error status

# Known bugs/edge cases
I know about these already:
1. If a recipe has multiple outputs then one of them will have a borked priority that's too high
2. When it tells you what you'll need to add to make the item you requested, it assumes highest priority recipes, and doesn't account for lower priority more efficient recipe paths. This is because we're assuming that the player will always want to use the highest priority path (ie they'd rather you tell them "add one more iron block to make an ingot" than "add 9 more nuggets to make an ingot" if they have the block recipe set as higher priority than the nugget recipe). This means that if a high efficiency low priority path exists, it will be ignored and you might be able to add less than the crafting plan says.
If you find anything else let me know and I'll add it to the list
