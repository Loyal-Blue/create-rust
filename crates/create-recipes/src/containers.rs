//! Generic container operations implemented by this port.
use super::{Heat, Ingredient, Product, Recipe};
pub fn recipes() -> &'static [Recipe] {
    &[
        Recipe {
            id: "create:filling/water_bucket",
            station: "filling",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:bucket",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:water",
                    count: 1000,
                    fluid: true,
                    consume: true,
                },
            ],
            products: &[Product {
                id: "minecraft:water_bucket",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:emptying/water_bucket",
            station: "emptying",
            ingredients: &[Ingredient {
                matcher: "minecraft:water_bucket",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:bucket",
                    count: 1,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:water",
                    count: 1000,
                    fluid: true,
                    chance: 1.0,
                },
            ],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:filling/lava_bucket",
            station: "filling",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:bucket",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:lava",
                    count: 1000,
                    fluid: true,
                    consume: true,
                },
            ],
            products: &[Product {
                id: "minecraft:lava_bucket",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:emptying/lava_bucket",
            station: "emptying",
            ingredients: &[Ingredient {
                matcher: "minecraft:lava_bucket",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:bucket",
                    count: 1,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:lava",
                    count: 1000,
                    fluid: true,
                    chance: 1.0,
                },
            ],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:filling/milk_bucket",
            station: "filling",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:bucket",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:milk",
                    count: 1000,
                    fluid: true,
                    consume: true,
                },
            ],
            products: &[Product {
                id: "minecraft:milk_bucket",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:emptying/milk_bucket",
            station: "emptying",
            ingredients: &[Ingredient {
                matcher: "minecraft:milk_bucket",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:bucket",
                    count: 1,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "minecraft:milk",
                    count: 1000,
                    fluid: true,
                    chance: 1.0,
                },
            ],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:filling/honey_bucket",
            station: "filling",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:bucket",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "create:honey",
                    count: 1000,
                    fluid: true,
                    consume: true,
                },
            ],
            products: &[Product {
                id: "create:honey_bucket",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:emptying/honey_bucket",
            station: "emptying",
            ingredients: &[Ingredient {
                matcher: "create:honey_bucket",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:bucket",
                    count: 1,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "create:honey",
                    count: 1000,
                    fluid: true,
                    chance: 1.0,
                },
            ],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:filling/chocolate_bucket",
            station: "filling",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:bucket",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "create:chocolate",
                    count: 1000,
                    fluid: true,
                    consume: true,
                },
            ],
            products: &[Product {
                id: "create:chocolate_bucket",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:emptying/chocolate_bucket",
            station: "emptying",
            ingredients: &[Ingredient {
                matcher: "create:chocolate_bucket",
                count: 1,
                fluid: false,
                consume: true,
            }],
            products: &[
                Product {
                    id: "minecraft:bucket",
                    count: 1,
                    fluid: false,
                    chance: 1.0,
                },
                Product {
                    id: "create:chocolate",
                    count: 1000,
                    fluid: true,
                    chance: 1.0,
                },
            ],
            work: 100,
            heat: Heat::None,
        },
        Recipe {
            id: "create:filling/water_bottle",
            station: "filling",
            ingredients: &[
                Ingredient {
                    matcher: "minecraft:glass_bottle",
                    count: 1,
                    fluid: false,
                    consume: true,
                },
                Ingredient {
                    matcher: "minecraft:water",
                    count: 250,
                    fluid: true,
                    consume: true,
                },
            ],
            products: &[Product {
                id: "minecraft:potion",
                count: 1,
                fluid: false,
                chance: 1.0,
            }],
            work: 100,
            heat: Heat::None,
        },
    ]
}
