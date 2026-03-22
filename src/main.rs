mod analysis;
mod domain;
mod planner;
mod scenario;
mod solver;

use domain::item_name;
use scenario::{get_recipes, get_starting_items, get_target};
use solver::{calculate_max, find_executable_solution_with_cycle_elimination, get_required_items};

fn main() {
    let recipes = get_recipes();
    let starting_items = get_starting_items();
    let target = get_target();

    let max = calculate_max(recipes.clone(), starting_items.clone(), target.clone());
    println!("Maximum number of first target item that can be crafted: {}", max);
    if max == 0 {
        println!("No solution found, cannot craft any of the target items with the provided recipes and starting items.");
        let required_items = get_required_items(recipes, starting_items, target);
        if required_items.items.is_empty() {
            println!("No additional base items identified by relaxed solve.");
        } else {
            println!("Required items to add to starting inventory:");
            for (item_id, count) in required_items.items {
                println!("- {}: {}", item_name(item_id), count);
            }
        }
        return;
    }

    let executable_or_fallback = find_executable_solution_with_cycle_elimination(
        recipes.clone(),
        starting_items.clone(),
        target.clone(),
    );

    if executable_or_fallback.is_err() {
        let disabled_recipe_ids = executable_or_fallback.err().unwrap_or_default();
        let fallback_recipes = recipes
            .iter()
            .filter(|recipe| !disabled_recipe_ids.contains(&recipe.unique_id))
            .cloned()
            .collect::<Vec<_>>();

        println!("No executable plan could be found after eliminating loop usage. Falling back to required-items analysis.");
        println!(
            "Required-items analysis will use {} recipes after cycle elimination.",
            fallback_recipes.len()
        );
        let required_items = get_required_items(fallback_recipes, starting_items, target);
        if required_items.items.is_empty() {
            println!("No additional base items identified by relaxed solve.");
        } else {
            println!("Required items to add to starting inventory:");
            for (item_id, count) in required_items.items {
                println!("- {}: {}", item_name(item_id), count);
            }
        }
        return;
    }

    let (solution, plan) = executable_or_fallback.unwrap();
    println!("Successfully crafted the target item!");

    println!("\nRecipe usage breakdown:");
    for recipe in &recipes {
        let var_value = solution.recipe_value(recipe);
        if var_value == 0.0 {
            continue;
        }
        println!("- {}: {}", recipe.name(), var_value);
    }

    println!("\nExecutable recipe plan:");
    for (recipe, count) in plan {
        println!("- Apply '{}' x{}", recipe.name(), count);
    }

    println!("\nFinal inventory:");
    for item in &solution.relevant_item_ids {
        let final_val = solution.final_inventory(*item);
        if final_val == 0.0 {
            continue;
        }
        println!("- {}: {}", item_name(*item), final_val);
    }
}
