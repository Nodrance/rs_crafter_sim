use good_lp::{constraint, default_solver, variable, variables, Expression, SolverModel, Solution, Variable};

enum ItemStack{
    StoragePart10(usize),
    StoragePart9(usize),
    StoragePart8(usize),
    StoragePart7(usize),
    StoragePart6(usize),
    StoragePart5(usize),
    StoragePart4(usize),
    StoragePart3(usize),
    StoragePart2(usize),
    StoragePart1(usize),
    Netherite(usize),
    Diamond(usize),
    Gold(usize),
    Iron(usize),
    Redstone(usize),
    Quartz(usize),
    Silicon(usize),
}

impl ItemStack {
    const COUNT: usize = 17;
    const fn all() -> [ItemStack; Self::COUNT] {
        [
            ItemStack::StoragePart10(0),
            ItemStack::StoragePart9(0),
            ItemStack::StoragePart8(0),
            ItemStack::StoragePart7(0),
            ItemStack::StoragePart6(0),
            ItemStack::StoragePart5(0),
            ItemStack::StoragePart4(0),
            ItemStack::StoragePart3(0),
            ItemStack::StoragePart2(0),
            ItemStack::StoragePart1(0),
            ItemStack::Netherite(0),
            ItemStack::Diamond(0),
            ItemStack::Gold(0),
            ItemStack::Iron(0),
            ItemStack::Redstone(0),
            ItemStack::Quartz(0),
            ItemStack::Silicon(0),
        ]
    }
    const fn id(&self) -> usize {
        match self {
            ItemStack::StoragePart10(_) => 0,
            ItemStack::StoragePart9(_) => 1,
            ItemStack::StoragePart8(_) => 2,
            ItemStack::StoragePart7(_) => 3,
            ItemStack::StoragePart6(_) => 4,
            ItemStack::StoragePart5(_) => 5,
            ItemStack::StoragePart4(_) => 6,
            ItemStack::StoragePart3(_) => 7,
            ItemStack::StoragePart2(_) => 8,
            ItemStack::StoragePart1(_) => 9,
            ItemStack::Netherite(_) => 10,
            ItemStack::Diamond(_) => 11,
            ItemStack::Gold(_) => 12,
            ItemStack::Iron(_) => 13,
            ItemStack::Redstone(_) => 14,
            ItemStack::Quartz(_) => 15,
            ItemStack::Silicon(_) => 16,
        }
    }
    const fn get_count(&self) -> usize {
        match self {
            ItemStack::StoragePart10(count) => *count,
            ItemStack::StoragePart9(count) => *count,
            ItemStack::StoragePart8(count) => *count,
            ItemStack::StoragePart7(count) => *count,
            ItemStack::StoragePart6(count) => *count,
            ItemStack::StoragePart5(count) => *count,
            ItemStack::StoragePart4(count) => *count,
            ItemStack::StoragePart3(count) => *count,
            ItemStack::StoragePart2(count) => *count,
            ItemStack::StoragePart1(count) => *count,
            ItemStack::Netherite(count) => *count,
            ItemStack::Diamond(count) => *count,
            ItemStack::Gold(count) => *count,
            ItemStack::Iron(count) => *count,
            ItemStack::Redstone(count) => *count,
            ItemStack::Quartz(count) => *count,
            ItemStack::Silicon(count) => *count,
        }
    }
    const fn name (&self) -> &'static str {
        match self {
            ItemStack::StoragePart10(_) => "StoragePart10",
            ItemStack::StoragePart9(_) => "StoragePart9",
            ItemStack::StoragePart8(_) => "StoragePart8",
            ItemStack::StoragePart7(_) => "StoragePart7",
            ItemStack::StoragePart6(_) => "StoragePart6",
            ItemStack::StoragePart5(_) => "StoragePart5",
            ItemStack::StoragePart4(_) => "StoragePart4",
            ItemStack::StoragePart3(_) => "StoragePart3",
            ItemStack::StoragePart2(_) => "StoragePart2",
            ItemStack::StoragePart1(_) => "StoragePart1",
            ItemStack::Netherite(_) => "Netherite",
            ItemStack::Diamond(_) => "Diamond",
            ItemStack::Gold(_) => "Gold",
            ItemStack::Iron(_) => "Iron",
            ItemStack::Redstone(_) => "Redstone",
            ItemStack::Quartz(_) => "Quartz",
            ItemStack::Silicon(_) => "Silicon",
        }
    }
}

struct ItemSet{
    items: Vec<ItemStack>,
}
struct Recipe{
    input: ItemSet,
    output: ItemSet,
}

fn main() {
    let recipe_names = [
        "StoragePart9 -> StoragePart10",
        "StoragePart8 -> StoragePart9",
        "StoragePart7 -> StoragePart8",
        "StoragePart6 -> StoragePart7",
        "StoragePart5 -> StoragePart6",
        "StoragePart4 -> StoragePart5",
        "StoragePart3 -> StoragePart4",
        "StoragePart2 -> StoragePart3",
        "StoragePart1 -> StoragePart2",
        "Base -> StoragePart1",
        "Quartz -> Silicon",
    ];

    let recipes = [
        // each storage part needs 3 of the previous, 4 silicon, 1 redstone, and netherite for 10/9, diamond for 8/7, gold for 6/5, iron for 4/3, quartz for 2/1
        Recipe{
            input: ItemSet{items: vec![ItemStack::StoragePart9(3), ItemStack::Silicon(4), ItemStack::Redstone(1), ItemStack::Netherite(1)]},
            output: ItemSet{items: vec![ItemStack::StoragePart10(1)]},
        },
        Recipe{
            input: ItemSet{items: vec![ItemStack::StoragePart8(3), ItemStack::Silicon(4), ItemStack::Redstone(1), ItemStack::Diamond(1)]},
            output: ItemSet{items: vec![ItemStack::StoragePart9(1)]},
        },
        Recipe{
            input: ItemSet{items: vec![ItemStack::StoragePart7(3), ItemStack::Silicon(4), ItemStack::Redstone(1), ItemStack::Diamond(1)]},
            output: ItemSet{items: vec![ItemStack::StoragePart8(1)]},
        },
        Recipe{
            input: ItemSet{items: vec![ItemStack::StoragePart6(3), ItemStack::Silicon(4), ItemStack::Redstone(1), ItemStack::Gold(1)]},
            output: ItemSet{items: vec![ItemStack::StoragePart7(1)]},
        },
        Recipe{
            input: ItemSet{items: vec![ItemStack::StoragePart5(3), ItemStack::Silicon(4), ItemStack::Redstone(1), ItemStack::Gold(1)]},
            output: ItemSet{items: vec![ItemStack::StoragePart6(1)]},
        },
        Recipe{
            input: ItemSet{items: vec![ItemStack::StoragePart4(3), ItemStack::Silicon(4), ItemStack::Redstone(1), ItemStack::Iron(1)]},
            output: ItemSet{items: vec![ItemStack::StoragePart5(1)]},
        },
        Recipe{
            input: ItemSet{items: vec![ItemStack::StoragePart3(3), ItemStack::Silicon(4), ItemStack::Redstone(1), ItemStack::Iron(1)]},
            output: ItemSet{items: vec![ItemStack::StoragePart4(1)]},
        },
        Recipe{
            input: ItemSet{items: vec![ItemStack::StoragePart2(3), ItemStack::Silicon(4), ItemStack::Redstone(1), ItemStack::Quartz(1)]},
            output: ItemSet{items: vec![ItemStack::StoragePart3(1)]},
        },
        Recipe{
            input: ItemSet{items: vec![ItemStack::StoragePart1(3), ItemStack::Silicon(4), ItemStack::Redstone(1), ItemStack::Quartz(1)]},
            output: ItemSet{items: vec![ItemStack::StoragePart2(1)]},
        },
        Recipe{
            input: ItemSet{items: vec![ItemStack::Silicon(4), ItemStack::Redstone(3), ItemStack::Quartz(2)]},
            output: ItemSet{items: vec![ItemStack::StoragePart1(1)]},
        },
        // silicon is made from smelting quartz
        Recipe{
            input: ItemSet{items: vec![ItemStack::Quartz(1)]},
            output: ItemSet{items: vec![ItemStack::Silicon(1)]},
        },
    ];

    let starting_items = [
        ItemStack::Netherite(10),
        ItemStack::Diamond(10),
        ItemStack::Gold(10),
        ItemStack::Iron(10),
        ItemStack::Redstone(100000),
        ItemStack::Quartz(100000),
    ];

    let target = ItemStack::StoragePart5(1);
    let target_amount = 1i32;

    let mut vars = variables!();
    let mut recipe_vars: Vec<Variable> = Vec::new();
    let mut constraints = Vec::new();
    // one variable for each recipe, representing how many times we use that recipe
    for _recipe in &recipes {
        recipe_vars.push(vars.add(variable().integer().min(0)));
    }
    // make sure that at the end, the total amount of items is positive
    // the total is the starting items plus the output of the recipes minus the input of the recipes
    for item in ItemStack::all() {
        let mut expr = Expression::from(0);
        for (i, recipe) in recipes.iter().enumerate() {
            let output_count = recipe.output.items.iter().find(|x| x.id() == item.id()).map_or(0, |x| x.get_count());
            let input_count = recipe.input.items.iter().find(|x| x.id() == item.id()).map_or(0, |x| x.get_count());
            expr = expr + (output_count as i32 - input_count as i32) * recipe_vars[i];
        }
        let starting_count = starting_items.iter().find(|x| x.id() == item.id()).map_or(0, |x| x.get_count());
        constraints.push(constraint!(expr + starting_count as i32 >= 0));
    }
    
    // constraint: we must produce at least target_amount of the target item
    let mut target_expr = Expression::from(0);
    for (i, recipe) in recipes.iter().enumerate() {
        let output_count = recipe.output.items.iter().find(|x| x.id() == target.id()).map_or(0, |x| x.get_count());
        let input_count = recipe.input.items.iter().find(|x| x.id() == target.id()).map_or(0, |x| x.get_count());
        target_expr = target_expr + (output_count as i32 - input_count as i32) * recipe_vars[i];
    }
    let starting_target_count = starting_items.iter().find(|x| x.id() == target.id()).map_or(0, |x| x.get_count());
    constraints.push(constraint!(target_expr.clone() + starting_target_count as i32 >= target_amount));
    
    // minimize the sum of all recipe uses to find the most efficient solution
    let mut objective = Expression::from(0);
    for recipe_var in &recipe_vars {
        objective = objective + recipe_var;
    }
    
    // solve the problem
    let problem = vars.minimise(objective).using(default_solver).with_all(constraints);
    match problem.solve() {
        Ok(solution) => {
            let final_target = target_expr + starting_target_count as i32;
            let final_target_count = solution.eval(&final_target);
            println!("Successfully crafted the target item!");
            println!("Final target item count: {}", final_target_count);

            println!("\nRecipe usage breakdown:");
            let mut used_any = false;
            for (i, recipe_var) in recipe_vars.iter().enumerate() {
                let uses = solution.value(*recipe_var).round() as i64;
                if uses > 0 {
                    println!("- {}: {}", recipe_names[i], uses);
                    used_any = true;
                }
            }
            if !used_any {
                println!("- No recipes needed!");
            }

            println!("\nRemaining inventory:");
            for item in ItemStack::all() {
                let item_id = item.id();
                let mut expr = Expression::from(0);
                for (i, recipe) in recipes.iter().enumerate() {
                    let output_count = recipe.output.items.iter().find(|x| x.id() == item_id).map_or(0, |x| x.get_count());
                    let input_count = recipe.input.items.iter().find(|x| x.id() == item_id).map_or(0, |x| x.get_count());
                    expr = expr + (output_count as i32 - input_count as i32) * recipe_vars[i];
                }
                let starting_count = starting_items.iter().find(|x| x.id() == item_id).map_or(0, |x| x.get_count());
                let final_count = solution.eval(&expr) as i32 + starting_count as i32;
                if final_count > 0 {
                    println!("- {}: {}", item.name(), final_count);
                }
            }
        }
        Err(e) => {
            println!("ERROR: Cannot craft the target item with current starting items!");
            println!("Reason: {:?}", e);
        }
    }
    
}
