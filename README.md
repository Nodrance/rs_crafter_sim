# How to set up
1. Install cargo and rust from https://www.rust-lang.org/tools/install
2. Install cmake or clang or whatever so that you can build highs (the solver I'm using) from source. Just install whatever it complains about missing when you try to build the project, and then try again until it works. Alternatively change cargo.toml from "highs" to "minilp".
# How to use
1. Edit `get_recipes()`, `get_starting_items()`, and `get_target()` in `src/demo_scenario.rs` to define your crafting scenario. (line 315 and below)
2. Run `cargo run` in your terminal

# Solver test harness
1. Edit JSON files in `tests/cases/` to add new test cases. See existing files for examples.
2. Run `cargo test --test solver_harness`.

# Known bugs/edge cases
I know about these already:
1. If a recipe has multiple outputs then one of them will have a borked priority that's too high
2. When it tells you what you'll need to add to make the item you requested, it assumes highest priority recipes, and doesn't account for lower priority more efficient recipe paths. This is because we're assuming that the player will always want to use the highest priority path (ie they'd rather you tell them "add one more iron block to make an ingot" than "add 9 more nuggets to make an ingot" if they have the block recipe set as higher priority than the nugget recipe). This means that if a high efficiency low priority path exists, it will be ignored and you might be able to add less than the crafting plan says.
3. Sometimes the solver will fail for some setups of cycles, mainly when you need to use a cycle several times in order to "unlock" a second cycle, such as a 1 nugget -> 2 nugget cycle and a 1 block -> 2 block cycle. 
If you find anything else let me know and I'll add it to the list.

# Things I am looking for:
1. Panics, anywhere
2. Incorrect max craftable output calculations
3. Incorrect crafting plans
4. Incorrect "add these items to make it craftable" in that adding those items doesn't make it craftable
5. Failing to use priority properly
6. Slow performance when there are only a few cycles or none
7. If you find something not on this list that feels like a bug, still let me know.