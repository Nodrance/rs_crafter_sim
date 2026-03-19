use std::cmp;

use good_lp::{constraint, default_solver, variable, variables, Expression, SolverModel, Solution, Variable};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
enum ItemType{
    Cobblestone,
    Gravel,
    Sand,
    Glass,
}

impl ItemType {
    const fn name(&self) -> &'static str {
        match self {
            ItemType::Cobblestone => "Cobblestone",
            ItemType::Gravel => "Gravel",
            ItemType::Sand => "Sand",
            ItemType::Glass => "Glass",
        }
    }
    const fn id(&self) -> u8 {
        *self as u8
    }
    const fn all() -> &'static [ItemType] {
        &[ItemType::Cobblestone, ItemType::Gravel, ItemType::Sand, ItemType::Glass]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ItemStack{
    item_type: ItemType,
    count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ItemSet{
    items: Vec<ItemStack>,
}

#[derive(Clone)]
struct Recipe{
    input: ItemSet,
    output: ItemSet,
    name: &'static str,
    priority: RecipePriority,
}

struct PrioritySolveResult {
    uses: Vec<i32>,
    final_target_count: i32,
    remaining_inventory: Vec<(ItemType, i32)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RecipePriority(Vec<isize>);

impl cmp::PartialOrd for RecipePriority {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl cmp::Ord for RecipePriority {
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


impl Recipe {
    fn new_single(input: ItemType, input_count: i32, output: ItemType, output_count: i32, priority: isize) -> Self {
        let name = format!("{} x{} -> {} x{}", input.name(), input_count, output.name(), output_count);
        Self {
            input: ItemSet { items: vec![ItemStack { item_type: input, count: input_count as usize }] },
            output: ItemSet { items: vec![ItemStack { item_type: output, count: output_count as usize }] },
            name: Box::leak(name.into_boxed_str()),
            priority: RecipePriority(vec![priority]),
        }
    }
}

fn sort_and_prune_recipes(recipes: &mut Vec<Recipe>, target_item_type: ItemType) {
    let item_count = ItemType::all().len();
    let mut producers_by_item: Vec<Vec<usize>> = vec![Vec::new(); item_count];

    for (recipe_index, recipe) in recipes.iter().enumerate() {
        for output in &recipe.output.items {
            producers_by_item[output.item_type.id() as usize].push(recipe_index);
        }
    }

    let base_priorities = recipes
        .iter()
        .map(|recipe| recipe.priority.0.first().copied().unwrap_or(0))
        .collect::<Vec<_>>();

    let mut effective_priorities: Vec<Option<RecipePriority>> = vec![None; recipes.len()];
    let mut stack: Vec<usize> = Vec::new();

    for &recipe_index in &producers_by_item[target_item_type.id() as usize] {
        let seed = RecipePriority(vec![base_priorities[recipe_index]]);
        effective_priorities[recipe_index] = Some(seed);
        stack.push(recipe_index);
    }

    while let Some(parent_index) = stack.pop() {
        let parent_priority = match &effective_priorities[parent_index] {
            Some(priority) => priority.clone(),
            None => continue,
        };

        for input in &recipes[parent_index].input.items {
            for &producer_index in &producers_by_item[input.item_type.id() as usize] {
                let mut candidate = parent_priority.clone();
                candidate.0.push(base_priorities[producer_index]);

                let should_update = match &effective_priorities[producer_index] {
                    Some(existing) => candidate > *existing,
                    None => true,
                };

                if should_update {
                    effective_priorities[producer_index] = Some(candidate);
                    stack.push(producer_index);
                }
            }
        }
    }

    let mut pruned_recipes = Vec::new();
    for (recipe_index, effective) in effective_priorities.into_iter().enumerate() {
        if let Some(priority) = effective {
            let mut recipe = recipes[recipe_index].clone();
            recipe.priority = priority;
            pruned_recipes.push(recipe);
        }
    }

    pruned_recipes.sort_by_key(|r| r.priority.clone());
    *recipes = pruned_recipes;
}

fn solve_with_priority_locks(
    recipes: &[Recipe],
    starting_items: &ItemSet,
    target: &ItemStack,
    priority_locks: &[(RecipePriority, i32)],
    objective_priority: Option<&RecipePriority>,
) -> Result<PrioritySolveResult, good_lp::ResolutionError> {
    let mut vars = variables!();
    let mut recipe_vars: Vec<Variable> = Vec::new();
    let mut constraints = Vec::new();

    for _recipe in recipes {
        recipe_vars.push(vars.add(variable().integer().min(0)));
    }

    for item_type in ItemType::all() {
        let mut expr = Expression::from(0);
        for (i, recipe) in recipes.iter().enumerate() {
            let output_count = recipe.output.items.iter()
                .find(|x| x.item_type == *item_type)
                .map_or(0, |x| x.count) as i32;
            let input_count = recipe.input.items.iter()
                .find(|x| x.item_type == *item_type)
                .map_or(0, |x| x.count) as i32;
            expr += (output_count - input_count) * recipe_vars[i];
        }
        let starting_count = starting_items.items.iter()
            .find(|x| x.item_type == *item_type)
            .map_or(0, |x| x.count) as i32;
        constraints.push(constraint!(expr + starting_count >= 0));
    }

    let mut target_expr = Expression::from(0);
    for (i, recipe) in recipes.iter().enumerate() {
        let output_count = recipe.output.items.iter()
            .find(|x| x.item_type == target.item_type)
            .map_or(0, |x| x.count) as i32;
        let input_count = recipe.input.items.iter()
            .find(|x| x.item_type == target.item_type)
            .map_or(0, |x| x.count) as i32;
        target_expr += (output_count - input_count) * recipe_vars[i];
    }
    let starting_target_count = starting_items.items.iter()
        .find(|x| x.item_type == target.item_type)
        .map_or(0, |x| x.count) as i32;
    constraints.push(constraint!(target_expr.clone() + starting_target_count >= target.count as i32));

    for (locked_priority, locked_sum) in priority_locks {
        let mut priority_expr = Expression::from(0);
        for (i, recipe) in recipes.iter().enumerate() {
            if &recipe.priority == locked_priority {
                priority_expr += recipe_vars[i];
            }
        }
        constraints.push(constraint!(priority_expr == *locked_sum));
    }

    let mut objective = Expression::from(0);
    if let Some(priority) = objective_priority {
        for (i, recipe) in recipes.iter().enumerate() {
            if &recipe.priority == priority {
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
    for item_type in ItemType::all() {
        let mut expr = Expression::from(0);
        for (i, recipe) in recipes.iter().enumerate() {
            let output_count = recipe.output.items.iter()
                .find(|x| x.item_type == *item_type)
                .map_or(0, |x| x.count) as i32;
            let input_count = recipe.input.items.iter()
                .find(|x| x.item_type == *item_type)
                .map_or(0, |x| x.count) as i32;
            expr += (output_count - input_count) * recipe_vars[i];
        }
        let starting_count = starting_items.items.iter()
            .find(|x| x.item_type == *item_type)
            .map_or(0, |x| x.count) as i32;
        let final_count = solution.eval(&expr) as i32 + starting_count;
        if final_count > 0 {
            remaining_inventory.push((*item_type, final_count));
        }
    }

    Ok(PrioritySolveResult {
        uses,
        final_target_count,
        remaining_inventory,
    })
}

fn main() {
    let mut recipes = vec![
        Recipe::new_single(
            ItemType::Cobblestone, 1, 
            ItemType::Gravel, 1,
            0),
        Recipe::new_single(
            ItemType::Gravel, 2,
            ItemType::Sand, 1,
            10),
        Recipe::new_single(
            ItemType::Sand, 4,
            ItemType::Glass, 2,
            10),
        Recipe::new_single(
            ItemType::Cobblestone, 10,
            ItemType::Glass, 9,
            5),
        Recipe::new_single(
            ItemType::Cobblestone, 1,
            ItemType::Cobblestone, 2,
            -10000),
    ];
    

    let starting_items = ItemSet { items: vec![
        ItemStack { item_type: ItemType::Cobblestone, count: 1 },
        ItemStack { item_type: ItemType::Gravel, count: 0 },
        ItemStack { item_type: ItemType::Sand, count: 0 },
        ItemStack { item_type: ItemType::Glass, count: 0 },
    ]};

    let target = ItemStack { item_type: ItemType::Glass, count: 11};

    sort_and_prune_recipes(&mut recipes, target.item_type);

    let feasibility = solve_with_priority_locks(&recipes, &starting_items, &target, &[], None);
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

    let mut priorities = recipes.iter().map(|r| r.priority.clone()).collect::<Vec<_>>();
    priorities.sort_unstable();
    priorities.dedup();

    let mut priority_locks: Vec<(RecipePriority, i32)> = Vec::new();

    for priority in &priorities {
        let step_result = solve_with_priority_locks(
            &recipes,
            &starting_items,
            &target,
            &priority_locks,
            Some(priority),
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
            .filter(|(_, recipe)| &recipe.priority == priority)
            .map(|(i, _)| uses[i])
            .sum::<i32>();

        priority_locks.push((priority.clone(), locked_sum));
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
            println!("- {}: {}", recipes[i].name, uses);
            used_any = true;
        }
    }
    if !used_any {
        println!("- No recipes needed!");
    }

    println!("\nRemaining inventory:");
    for (item_type, count) in final_inventory {
        println!("- {}: {}", item_type.name(), count);
    }
    
}
