# How to use
0. Install cargo and rust from https://www.rust-lang.org/tools/install
1. Edit `get_recipes()`, `get_starting_items()`, and `get_target()` in `src/main.rs` to define your crafting scenario. (line 315 and below)
2. Run `cargo run` in your terminal

# Known bugs/edge cases
I know about these already:
1. If a recipe has multiple outputs then one of them will have a borked priority that's too high
2. It doesn't check if you can start loops. If you have something like 1->2 cobblestone, it'll say "okay great that's the same as 0->1, so just make a cobble out of thin air
3. If you can't make the target it will throw an error instead of telling you what you'd need to get in order to make it. That's the next step
If you find anything else let me know and I'll add it to the list
