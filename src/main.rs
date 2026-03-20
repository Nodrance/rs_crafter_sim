use std::{cmp, collections::{HashMap, VecDeque}};
use good_lp::{constraint, default_solver, variable, variables, Expression, SolverModel, Solution, Variable};

type ItemId = usize;
const COBBLESTONE_ID: ItemId = 0;
const GRAVEL_ID: ItemId = 1;
const SAND_ID: ItemId = 2;
const GLASS_ID: ItemId = 3;
const DIAMOND_ID: ItemId = 4;

const ITEM_NAMES: [&str; 5] = ["Cobblestone", "Gravel", "Sand", "Glass", "Diamond"];
fn item_name(item_id: ItemId) -> &'static str {
    ITEM_NAMES.get(item_id).copied().unwrap_or("Unknown")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ItemStack{
    item_id: ItemId,
    count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ItemSet{
    items: Vec<ItemStack>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Recipe{
    input: ItemSet,
    output: ItemSet,
    base_priority: isize,
    effective_priority: Option<isize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct PrioritySolveResult {
    uses: Vec<i32>,
    final_target_count: i32,
    remaining_inventory: Vec<(ItemId, i32)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RecipePriorityKey(Vec<isize>);
impl cmp::PartialOrd for RecipePriorityKey {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl cmp::Ord for RecipePriorityKey {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        for (a, b) in self.0.iter().zip(other.0.iter()) {
            match a.cmp(b) {
                cmp::Ordering::Equal => {}
                non_equal => return non_equal,
            }
        }
        self.0.len().cmp(&other.0.len())
    }
}
impl RecipePriorityKey {
    fn add_recipe(&mut self, recipe: &Recipe) {
        self.0.push(recipe.base_priority);
    }
}

impl Recipe {
    fn new_single(input: ItemId, input_count: i32, output: ItemId, output_count: i32, priority: isize) -> Self {
        Self {
            input: ItemSet { items: vec![ItemStack { item_id: input, count: input_count as usize }] },
            output: ItemSet { items: vec![ItemStack { item_id: output, count: output_count as usize }] },
            base_priority: priority,
            effective_priority: None,
        }
    }

    fn name(&self) -> String {
        let input_str = self.input.items.iter()
            .map(|stack| format!("{} x{}", item_name(stack.item_id), stack.count))
            .collect::<Vec<_>>()
            .join(" + ");
        let output_str = self.output.items.iter()
            .map(|stack| format!("{} x{}", item_name(stack.item_id), stack.count))
            .collect::<Vec<_>>()
            .join(" + ");
        format!("{} -> {}", input_str, output_str)
    }
}

fn sort_and_prune_recipes(recipes: Vec<Recipe>, target_item_id: ItemId) -> (Vec<Recipe>, Vec<ItemId>) {
    // Each item inherits the best priority key of any recipe that produces it
    // Each recipe inherits the priority key of its best output item type, plus its own base priority
    // This continues until it stabilizes (because loops are possible but will never result in a better key)
    // Then we return only recipes and item types that were given a priority key, because those are the only ones that could possibly be relevant to crafting the target item type
    // We also set each recipe's effective priority to an isize that represents its position in the final sorted order, so we can easily lock priorities later when solving
    // We also make items "relevant" if they are produced by a relevant recipe, to make sure we keep byproducts
    let mut best_item_priorities: HashMap<ItemId, RecipePriorityKey> = HashMap::new();
    let mut best_recipe_priorities: HashMap<Recipe, Option<RecipePriorityKey>> = HashMap::new();
    let mut queue = VecDeque::new();

    best_item_priorities.insert(target_item_id, RecipePriorityKey(Vec::new()));
    queue.push_back(target_item_id);

    while let Some(output_item_id) = queue.pop_front() {
        let output_priority = best_item_priorities.get(&output_item_id)
            .cloned()
            .expect("queued items must already have a priority");

        for recipe in recipes.iter() {
            if !recipe.output.items.iter().any(|item| item.item_id == output_item_id) {
                continue;
            }

            let mut candidate_recipe_priority = output_priority.clone();
            candidate_recipe_priority.add_recipe(recipe);

            let should_update_recipe = best_recipe_priorities.get(recipe)
                .and_then(|opt| opt.as_ref())
                .is_none_or(|current| candidate_recipe_priority < *current);

            if !should_update_recipe {
                continue;
            }

            best_recipe_priorities.insert(recipe.clone(), Some(candidate_recipe_priority.clone()));

            for input in &recipe.input.items {
                let should_update_item = best_item_priorities
                    .get(&input.item_id)
                    .is_none_or(|current| candidate_recipe_priority < *current);

                if should_update_item {
                    best_item_priorities.insert(input.item_id, candidate_recipe_priority.clone());
                    queue.push_back(input.item_id);
                }
            }
        }
    }

    let mut sorted_recipes = best_recipe_priorities
        .into_iter()
        .filter_map(|(recipe, priority)| priority.map(|priority| (priority, recipe)))
        .collect::<Vec<_>>();
    sorted_recipes.sort_by(|(left_priority, left_recipe), (right_priority, right_recipe)| {
        left_priority
            .cmp(right_priority)
            .then_with(|| left_recipe.name().cmp(&right_recipe.name()))
    });

    let pruned_recipes = sorted_recipes
        .into_iter()
        .enumerate()
        .map(|(index, (_priority, mut recipe))| {
            recipe.effective_priority = Some(index as isize);
            recipe
        })
        .collect::<Vec<_>>();

    let mut relevant_item_ids = Vec::new();
    for recipe in &pruned_recipes {
        for item in &recipe.output.items {
            if !relevant_item_ids.contains(&item.item_id) {
                relevant_item_ids.push(item.item_id);
            }
        }
        for item in &recipe.input.items {
            if !relevant_item_ids.contains(&item.item_id) {
                relevant_item_ids.push(item.item_id);
            }
        }
    }

    (pruned_recipes, relevant_item_ids)
}

fn solve_with_priority_locks(
    recipes: &[Recipe],
    relevant_item_ids: &[ItemId],
    starting_items: &ItemSet,
    target: &ItemStack,
    priority_locks: &[(isize, i32)],
    objective_priority: Option<isize>,
) -> Result<PrioritySolveResult, good_lp::ResolutionError> {
    let mut vars = variables!();
    let mut recipe_vars: Vec<Variable> = Vec::new();
    let mut constraints = Vec::new();

    for _recipe in recipes {
        recipe_vars.push(vars.add(variable().integer().min(0)));
    }

    for item_id in relevant_item_ids {
        let mut expr = Expression::from(0);
        for (i, recipe) in recipes.iter().enumerate() {
            let output_count = recipe.output.items.iter()
                .find(|x| x.item_id == *item_id)
                .map_or(0, |x| x.count) as i32;
            let input_count = recipe.input.items.iter()
                .find(|x| x.item_id == *item_id)
                .map_or(0, |x| x.count) as i32;
            expr += (output_count - input_count) * recipe_vars[i];
        }
        let starting_count = starting_items.items.iter()
            .find(|x| x.item_id == *item_id)
            .map_or(0, |x| x.count) as i32;
        constraints.push(constraint!(expr + starting_count >= 0));
    }

    let mut target_expr = Expression::from(0);
    for (i, recipe) in recipes.iter().enumerate() {
        let output_count = recipe.output.items.iter()
            .find(|x| x.item_id == target.item_id)
            .map_or(0, |x| x.count) as i32;
        let input_count = recipe.input.items.iter()
            .find(|x| x.item_id == target.item_id)
            .map_or(0, |x| x.count) as i32;
        target_expr += (output_count - input_count) * recipe_vars[i];
    }
    let starting_target_count = starting_items.items.iter()
        .find(|x| x.item_id == target.item_id)
        .map_or(0, |x| x.count) as i32;
    constraints.push(constraint!(target_expr.clone() + starting_target_count >= target.count as i32));

    for (locked_priority, locked_sum) in priority_locks {
        let mut priority_expr = Expression::from(0);
        for (i, recipe) in recipes.iter().enumerate() {
            if recipe.effective_priority == Some(*locked_priority) {
                priority_expr += recipe_vars[i];
            }
        }
        constraints.push(constraint!(priority_expr == *locked_sum));
    }

    let mut objective = Expression::from(0);
    if let Some(priority) = objective_priority {
        for (i, recipe) in recipes.iter().enumerate() {
            if recipe.effective_priority == Some(priority) {
                objective += recipe_vars[i];
            }
        }
    }

    let problem = vars.minimise(objective).using(default_solver).with_all(constraints);
    let solution = problem.solve()?;

    let uses = recipe_vars
        .iter()
        .map(|v| solution.value(*v).round() as i32)
        .collect::<Vec<_>>();

    let final_target_count = solution.eval(&target_expr) as i32 + starting_target_count;

    let mut remaining_inventory = Vec::new();
    for item_id in relevant_item_ids {
        let mut expr = Expression::from(0);
        for (i, recipe) in recipes.iter().enumerate() {
            let output_count = recipe.output.items.iter()
                .find(|x| x.item_id == *item_id)
                .map_or(0, |x| x.count) as i32;
            let input_count = recipe.input.items.iter()
                .find(|x| x.item_id == *item_id)
                .map_or(0, |x| x.count) as i32;
            expr += (output_count - input_count) * recipe_vars[i];
        }
        let starting_count = starting_items.items.iter()
            .find(|x| x.item_id == *item_id)
            .map_or(0, |x| x.count) as i32;
        let final_count = solution.eval(&expr) as i32 + starting_count;
        if final_count > 0 {
            remaining_inventory.push((*item_id, final_count));
        }
    }

    Ok(PrioritySolveResult {
        uses,
        final_target_count,
        remaining_inventory,
    })
}

fn main() {
    let recipes = vec![
        Recipe::new_single(
            COBBLESTONE_ID, 1, 
            GRAVEL_ID, 1,
            0),
        Recipe::new_single(
            GRAVEL_ID, 2,
            SAND_ID, 1,
            10),
        Recipe::new_single(
            SAND_ID, 4,
            GLASS_ID, 2,
            10),
        Recipe::new_single(
            COBBLESTONE_ID, 10,
            GLASS_ID, 9,
            5),
        // 1 cobblestone into 2 cobblestone and a diamond, negative 100000 priority
        Recipe{
            input: ItemSet { items: vec![ItemStack { item_id: COBBLESTONE_ID, count: 1 }] },
            output: ItemSet { items: vec![ItemStack { item_id: COBBLESTONE_ID, count: 2 }, ItemStack { item_id: DIAMOND_ID, count: 1 }] },
            base_priority: -100000,
            effective_priority: None,
        }
    ];
    

    let starting_items = ItemSet { items: vec![
        ItemStack { item_id: COBBLESTONE_ID, count: 1 }
    ]};

    let target = ItemStack { item_id: GLASS_ID, count: 11};

    let (recipes, relevant_item_ids) = sort_and_prune_recipes(recipes, target.item_id);

    let feasibility = solve_with_priority_locks(&recipes, &relevant_item_ids, &starting_items, &target, &[], None);
    let PrioritySolveResult {
        uses: mut final_uses,
        mut final_target_count,
        remaining_inventory: mut final_inventory,
    } = match feasibility {
        Ok(result) => result,
        Err(e) => {
            println!("ERROR: Cannot craft the target item with current starting items!");
            println!("Reason: {:?}", e);
            return;
        }
    };

    let mut priorities = recipes.iter().filter_map(|r| r.effective_priority).collect::<Vec<_>>();
    priorities.sort_unstable();
    priorities.dedup();

    let mut priority_locks: Vec<(isize, i32)> = Vec::new();

    for priority in &priorities {
        let step_result = solve_with_priority_locks(
            &recipes,
            &relevant_item_ids,
            &starting_items,
            &target,
            &priority_locks,
            Some(*priority),
        );

        let PrioritySolveResult {
            uses,
            final_target_count: target_count,
            remaining_inventory: inventory,
        } = match step_result {
            Ok(result) => result,
            Err(e) => {
                println!("ERROR: Failed while recursively minimizing priorities!");
                println!("Reason: {:?}", e);
                return;
            }
        };

        let locked_sum = recipes
            .iter()
            .enumerate()
            .filter(|(_, recipe)| recipe.effective_priority == Some(*priority))
            .map(|(i, _)| uses[i])
            .sum::<i32>();

        priority_locks.push((*priority, locked_sum));
        final_uses = uses;
        final_target_count = target_count;
        final_inventory = inventory;
    }

    println!("Successfully crafted the target item!");
    println!("Final target item count: {}", final_target_count);

    println!("\nRecipe usage breakdown:");
    let mut used_any = false;
    for (i, uses) in final_uses.iter().enumerate() {
        if *uses > 0 {
            println!("- {}: {}", recipes[i].name(), uses);
            used_any = true;
        }
    }
    if !used_any {
        println!("- No recipes needed!");
    }

    println!("\nRemaining inventory:");
    for (item_id, count) in final_inventory {
        println!("- {}: {}", item_name(item_id), count);
    }
    
}
