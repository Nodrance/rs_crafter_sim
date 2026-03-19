import pulp

# ----------------------------
# DATA (Easily extensible)
# ----------------------------

# List of items
items = [f"storage_part_{i}" for i in range(1, 33)] + ["netherite", "diamond", "gold", "iron", "redstone", "quartz", "silicon"]

# Starting quantities
start = {
    "netherite": 2147483647,
    "diamond": 2147483647,
    "gold": 2147483647,
    "iron": 2147483647,
    "redstone": 2147483647,
    "quartz": 2147483647,
    "silicon": 0,
}

# Target requirements (omit or set to 0 if none)
target = {
    "storage_part_18": 1,
}

# Recipes:
# Each recipe is a dict: item -> net change
# (produced - consumed)
# Recipe{
#             input: ItemSet{items: vec![ItemStack::StoragePart1(3), ItemStack::Silicon(4), ItemStack::Redstone(1), ItemStack::Quartz(1)]},
#             output: ItemSet{items: vec![ItemStack::StoragePart2(1)]},
#         },
#         Recipe{
#             input: ItemSet{items: vec![ItemStack::Silicon(4), ItemStack::Redstone(3), ItemStack::Quartz(2)]},
#             output: ItemSet{items: vec![ItemStack::StoragePart1(1)]},
#         },
#         // silicon is made from smelting quartz
#         Recipe{
#             input: ItemSet{items: vec![ItemStack::Quartz(1)]},
#             output: ItemSet{items: vec![ItemStack::Silicon(1)]},
#         },
recipes = [
    {"silicon": 1, "quartz": -1},
    {"storage_part_1": 1, "silicon": -4, "redstone": -3, "quartz": -2},
] + [
    {f"storage_part_{i+1}": 1, f"storage_part_{i}": -3, "silicon": -4, "redstone": -1, "iron": -1}
    for i in range(0, 8)
] + [
    {f"storage_part_{i+1}": 1, f"storage_part_{i}": -3, "silicon": -4, "redstone": -1, "gold": -1}
    for i in range(8, 16)
] + [
    {f"storage_part_{i+1}": 1, f"storage_part_{i}": -3, "silicon": -4, "redstone": -1, "diamond": -1}
    for i in range(16, 24)
] + [
    {f"storage_part_{i+1}": 1, f"storage_part_{i}": -3, "silicon": -4, "redstone": -1, "netherite": -1}
    for i in range(24, 32)
]


# ----------------------------
# BUILD MODEL
# ----------------------------

prob = pulp.LpProblem("crafting", pulp.LpMinimize)

# Variables: one per recipe
x = [
    pulp.LpVariable(f"R{j}", lowBound=0, cat="Integer")
    for j in range(len(recipes))
]

# Objective 1: minimize total recipe usage
prob += pulp.lpSum(x)

# ----------------------------
# CONSTRAINTS
# ----------------------------

# Item balance constraints
for item in items:
    prob += (
        start.get(item, 0) +
        pulp.lpSum(recipes[j].get(item, 0) * x[j] for j in range(len(recipes)))
        >= 0
    ), f"nonnegative_{item}"

# Target constraints
for item, T in target.items():
    prob += (
        start.get(item, 0) +
        pulp.lpSum(recipes[j].get(item, 0) * x[j] for j in range(len(recipes)))
        >= T
    ), f"target_{item}"

# ----------------------------
# STAGE 1: minimize total usage
# ----------------------------

prob.solve()

S_star = sum(v.value() for v in x)

# Fix total usage
prob += pulp.lpSum(x) == S_star, "fix_total_usage"

# ----------------------------
# STAGE 2: lexicographic maximize
# ----------------------------

for j in range(len(recipes)):
    prob.sense = pulp.LpMaximize
    prob.setObjective(x[j])
    prob.solve()

    val = x[j].value()
    prob += x[j] == val, f"fix_R{j}"

# ----------------------------
# OUTPUT
# ----------------------------

print("Recipe usage:")
for j in range(len(recipes)):
    print(f"R{j}:", int(x[j].value()))

print("\nFinal item counts:")
for item in items:
    final_val = (
        sum(recipes[j].get(item, 0) * int(x[j].value()) for j in range(len(recipes)))
    )
    print(f"{item}:", int(final_val))