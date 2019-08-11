use rand::distributions::Weighted;
use tcod::{
    colors,
};
use crate::{
    types::*,
    ai::Ai,
    util::{Transition, from_dungeon_level},
};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Monster {
    Orc,
    Troll,
}

pub fn make_monster(x: i32, y: i32, monster: Monster) -> Object {
    let mut monster = match monster {
        Monster::Orc => {
            let mut orc = Object::new(x, y, 'o', colors::DESATURATED_GREEN, "Orc", true);
            orc.fighter = Some(Fighter{
                base_max_hp: 10,
                hp: 10,
                base_defense: 0,
                base_power: 3,
                on_death: DeathCallback::Monster,
                xp: 35,
                base_movement: 4,
            });
            orc.ai = Some(Ai::Basic);

            orc
        },

        Monster::Troll => {
            let mut troll = Object::new(x, y, 'T', colors::DARKER_GREEN, "Troll", true);
            troll.fighter = Some(Fighter {
                base_max_hp: 16,
                hp: 16,
                base_defense: 1,
                base_power: 4,
                on_death: DeathCallback::Monster,
                xp: 100,
                base_movement: 3,
            });
            troll.ai = Some(Ai::Basic);

            troll
        },
    };

    monster.alive = true;
    monster.always_visible = true;

    monster
}

pub fn monster_table_for_level(level: u32) -> Vec<Weighted<Monster>> {
    let troll_chance = from_dungeon_level(
        &[
            Transition {
                level: 3,
                value: 15,
            },
            Transition {
                level: 5,
                value: 30,
            },
            Transition {
                level: 7,
                value: 60,
            },
        ],
        level,
    );

    vec![
        Weighted {
            weight: 80,
            item: Monster::Orc,
        },
        Weighted {
            weight: troll_chance,
            item: Monster::Troll,
        }
    ]
}
