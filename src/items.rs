use crate::{
    consts::*,
    types::*,
    logging::*,
    ai::Ai,
    util::{Transition, from_dungeon_level},

    target_monster,
    closest_monster,
    target_tile,
};
use tcod::{
    colors,
};
use rand::{distributions::{IndependentSample, Weighted, WeightedChoice}, Rng};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub enum Item {
    Heal,
    Lightning,
    Confuse,
    Fireball,
    Sword,
    Shield,
}

pub fn make_item(x: i32, y: i32, item: Item) -> Object {
    match item {
        Item::Heal => {
            let mut object = Object::new(
                x,
                y,
                '!',
                colors::VIOLET,
                "healing potion",
                false,
            );
            object.item = Some(Item::Heal);
            object
        },
        Item::Lightning => {
            let mut object = Object::new(
                x,
                y,
                '#',
                colors::LIGHT_YELLOW,
                "scroll of lightning bolt",
                false,
            );
            object.item = Some(Item::Lightning);
            object
        },
        Item::Fireball => {
            let mut object = Object::new(
                x,
                y,
                '#',
                colors::LIGHT_YELLOW,
                "scroll of fireball",
                false,
            );
            object.item = Some(Item::Fireball);
            object
        },
        Item::Confuse => {
            let mut object = Object::new(
                x,
                y,
                '#',
                colors::LIGHT_YELLOW,
                "scroll of confusion",
                false,
            );
            object.item = Some(Item::Confuse);
            object
        },
        Item::Sword => {
            let sword = Equipment {
                equipped: false,
                slot: Slot::RightHand,
                power_bonus: 3,
                defense_bonus: 0,
                max_hp_bonus: 0,
            };
            let mut object = Object::new(x, y, '/', colors::SKY, "sword", false);
            object.item = Some(Item::Sword);
            object.equipment = Some(sword);
            object
        },
        Item::Shield => {
            let shield = Equipment {
                equipped: false,
                slot: Slot::LeftHand,
                power_bonus: 0,
                defense_bonus: 1,
                max_hp_bonus: 0,
            };
            let mut object = Object::new(x, y, '[', colors::SKY, "shield", false);
            object.item = Some(Item::Shield);
            object.equipment = Some(shield);
            object
        }
    }
}

pub fn item_table_for_level(level: u32) -> Vec<Weighted<Item>> {
    vec![
        Weighted {
            weight: 35,
            item: Item::Heal,
        },
        Weighted {
            weight: from_dungeon_level(
                &[Transition {
                    level: 4,
                    value: 25,
                }],
                level,
            ),
            item: Item::Lightning,
        },
        Weighted {
            weight: from_dungeon_level(
                &[Transition {
                    level: 6,
                    value: 25,
                }],
                level,
            ),
            item: Item::Fireball,
        },
        Weighted {
            weight: from_dungeon_level(
                &[Transition {
                    level: 2,
                    value: 10
                }],
                level,
            ),
            item: Item::Confuse,
        },
        Weighted {
            weight: from_dungeon_level(
                &[Transition {
                    level: 4,
                    value: 5,
                }],
                level,
            ),
            item: Item::Sword,
        },
        Weighted {
            weight: from_dungeon_level(
                &[Transition {
                    level: 8,
                    value: 15,
                }],
                level,
            ),
            item: Item::Shield,
        },
    ]
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Equipment {
    pub slot: Slot,
    pub equipped: bool,
    pub power_bonus: i32,
    pub defense_bonus: i32,
    pub max_hp_bonus: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Slot {
    LeftHand,
    RightHand,
    Head,
}

impl std::fmt::Display for Slot {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            Slot::LeftHand => write!(f, "left hand"),
            Slot::RightHand => write!(f, "right hand"),
            Slot::Head => write!(f, "head"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UseResult {
    UsedUp,
    Cancelled,
    UsedAndKept,
}


pub fn use_item(
    inventory_id: usize,
    objects: &mut [Object],
    game: &mut Game,
    tcod: &mut Tcod,
) {
    if let Some(item) = game.inventory[inventory_id].item {
        let on_use = match item {
            Item::Heal => cast_heal,
            Item::Lightning => cast_lightning,
            Item::Confuse => cast_confuse,
            Item::Fireball => cast_fireball,
            Item::Sword => toggle_equipment,
            Item::Shield => toggle_equipment,
        };

        match on_use(inventory_id, objects, game, tcod) {
            UseResult::UsedUp => {
                game.inventory.remove(inventory_id);
            }
            UseResult::Cancelled => {
                game.log.gutter_text("Cancelled", colors::WHITE);
            }
            UseResult::UsedAndKept => {}
        }
    } else {
        game.log.gutter_text(
            format!("The {} cannot be used.", game.inventory[inventory_id].name),
            colors::WHITE
        );
    }
}

pub fn toggle_equipment(
    inventory_id: usize,
    _objects: &mut [Object],
    game: &mut Game,
    _tcod: &mut Tcod,
) -> UseResult {
    let equipment = match game.inventory[inventory_id].equipment {
        Some(equipment) => equipment,
        None => return UseResult::Cancelled,
    };

    if let Some(old_equipment) = get_equipped_in_slot(equipment.slot, &game.inventory) {
        game.inventory[old_equipment].dequip(&mut game.log);
    }

    if equipment.equipped {
        game.inventory[inventory_id].dequip(&mut game.log);
    } else {
        game.inventory[inventory_id].equip(&mut game.log);
    }

    UseResult::UsedAndKept
}

pub fn pick_item_up(
    object_id: usize,
    objects: &mut Vec<Object>,
    game: &mut Game,
) {
    if game.inventory.len() >= 26 {
        game.log.gutter_text(
            format!("Your inventory is full, cannot pick up {}.", objects[object_id].name),
            colors::RED,
        )
    } else {
        let item = objects.swap_remove(object_id);

        game.log.gutter_text(
            format!("You picked up a {}!", item.name),
            colors::GREEN,
        );
        let index = game.inventory.len();
        let slot = item.equipment.map(|e| e.slot);
        game.inventory.push(item);

        if let Some(slot) = slot {
            if get_equipped_in_slot(slot, &game.inventory).is_none() {
                game.inventory[index].equip(&mut game.log);
            }
        }
    }
}

pub fn drop_item(
    inventory_id: usize,
    game: &mut Game,
    objects: &mut Vec<Object>,
) {
    let mut item = game.inventory.remove(inventory_id);

    item.set_pos(objects[PLAYER].x, objects[PLAYER].y);

    game.log.gutter_text(
        format!("You dropped a {}.", item.name),
        colors::YELLOW,
    );

    objects.push(item);
}

pub fn get_equipped_in_slot(slot: Slot, inventory: &[Object]) -> Option<usize> {
    for (inventory_id, item) in inventory.iter().enumerate() {
        if item
            .equipment
            .as_ref()
            .map_or(false, |e| e.equipped && e.slot == slot)
        {
            return Some(inventory_id);
        }
    }

    None
}

fn cast_heal(
    _inventory_id: usize,
    objects: &mut [Object],
    game: &mut Game,
    tcod: &mut Tcod,
) -> UseResult {
    if let Some(fighter) = objects[PLAYER].fighter {
        if fighter.hp == objects[PLAYER].max_hp(game) {
            game.log.gutter_text(
                "You are already at full health.",
                colors::RED,
            );

            return UseResult::Cancelled;
        }
        game.log.gutter_text(
            "Your wounds start to feel better!",
            colors::LIGHT_VIOLET,
        );

        objects[PLAYER].heal(HEAL_AMOUNT, game);
        return UseResult::UsedUp;
    }

    UseResult::Cancelled
}

fn cast_lightning(
    _inventory_id: usize,
    objects: &mut [Object],
    game: &mut Game,
    tcod: &mut Tcod,
) -> UseResult {
    let monster_id = closest_monster(LIGHTNING_RANGE, objects, tcod);

    if let Some(monster_id) = monster_id {
        game.log.gutter_text(
            format!(
                "The lightning bolt strikes the {} with a loud thunder! \
                 The damage is {} hit points.",
                objects[monster_id].name, LIGHTNING_DAMAGE
            ),
            colors::LIGHT_BLUE,
        );

        if let Some(xp) = objects[monster_id].take_damage(LIGHTNING_DAMAGE, game) {
            objects[PLAYER].fighter.as_mut().unwrap().xp += xp;
        }

        UseResult::UsedUp
    } else {
        game.log.gutter_text("No enemy is close enough to strike,", colors::RED);
        UseResult::Cancelled
    }
}

fn cast_fireball(
    _inventory_id: usize,
    objects: &mut [Object],
    game: &mut Game,
    tcod: &mut Tcod,
) -> UseResult {
    game.log.gutter_text(
        "Left-click a target tile for the fireball, or right-clock to cancel.",
        colors::LIGHT_CYAN,
    );

    let (x, y) = match target_tile(tcod, objects, game, None) {
        Some(tile_pos) => tile_pos,
        None => return UseResult::Cancelled,
    };
    game.log.gutter_text(
        format!(
            "The fireball explodes, burning everything within {} tiles!",
            FIREBALL_RADIUS,
        ),
        colors::ORANGE,
    );

    let mut xp_to_gain = 0;
    for (id, obj) in objects.iter_mut().enumerate() {
        if obj.distance(x, y) <= FIREBALL_RADIUS as f32 && obj.fighter.is_some() {
            game.log.gutter_text(
                format!(
                    "The {} gets burned for {} hitpoints.",
                    obj.name, FIREBALL_DAMAGE
                ),
                colors::ORANGE,
            );

            if let Some(xp) = obj.take_damage(FIREBALL_DAMAGE, game) {
                if id != PLAYER {
                    xp_to_gain += xp;
                }
            }
        }
    }

    objects[PLAYER].fighter.as_mut().unwrap().xp += xp_to_gain;

    UseResult::UsedUp
}

fn cast_confuse(
    _inventory_id: usize,
    objects: &mut [Object],
    game: &mut Game,
    tcod: &mut Tcod,
) -> UseResult {
    let monster_id = target_monster(tcod, objects, game, Some(CONFUSE_RANGE as f32));

    if let Some(monster_id) = monster_id {
        let old_ai = objects[monster_id].ai.take().unwrap_or(Ai::Basic);

        objects[monster_id].ai = Some(Ai::Confused {
            previous_ai: Box::new(old_ai),
            num_turns: CONFUSE_NUM_TURNS,
        });

        game.log.gutter_text(
            format!(
                "The eyes of {} look vacant, as he starts to stumble around!",
                objects[monster_id].name
            ),
            colors::LIGHT_GREEN,
        );

        UseResult::UsedUp
    } else {
        game.log.gutter_text(
            "No enemy is close enough to strike.",
            colors::RED
        );
        UseResult::Cancelled
    }
}
