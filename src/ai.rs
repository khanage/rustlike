use crate::{
    types::*,
    consts::*,
    logging::*,
    util::mut_two,
    is_blocked,
};
use tcod::{
    colors,
    map::{Map as FovMap},
};
use rand::Rng;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum Ai {
    Basic,
    Confused {
        previous_ai: Box<Ai>,
        num_turns: i32,
    },
}

pub fn ai_take_turn(
    monster_id: usize,
    game: &mut Game,
    objects: &mut [Object],
    fov_map: &FovMap,
) {
    use Ai::*;

    if let Some(ai) = objects[monster_id].ai.take() {
        let new_ai = match ai {
            Basic => ai_basic(monster_id, game, objects, fov_map),
            Confused {
                previous_ai,
                num_turns,
            } => ai_confused(monster_id, game, objects, previous_ai, num_turns),
        };

        objects[monster_id].ai = Some(new_ai);
    }
}

pub fn ai_basic(
    monster_id: usize,
    game: &mut Game,
    objects: &mut [Object],
    fov_map: &FovMap,
) -> Ai {
    let (monster_x, monster_y) = objects[monster_id].pos();

    if fov_map.is_in_fov(monster_x, monster_y) {
        if objects[monster_id].distance_to(&objects[PLAYER]) >= 2.0 {
            let (player_x, player_y) = objects[PLAYER].pos();
            move_towards(monster_id, player_x, player_y, &game.map, objects);
        } else if objects[PLAYER].fighter.map_or(false, |f| f.hp > 0) {
            attack(monster_id, PLAYER, objects, game);
        }
    }

    Ai::Basic
}

pub fn ai_confused(
    monster_id: usize,
    game: &mut Game,
    objects: &mut [Object],
    previous_ai: Box<Ai>,
    num_turns: i32,
) -> Ai {
    if num_turns >= 0 {
        move_by(
            monster_id,
            rand::thread_rng().gen_range(-1, 2),
            rand::thread_rng().gen_range(-1, 2),
            &game.map,
            objects,
        );
        Ai::Confused {
            previous_ai: previous_ai,
            num_turns: num_turns - 1,
        }
    } else {
        game.log.gutter_text(
            format!("The {} is no longer confused.", objects[monster_id].name),
            colors::RED,
        );

        *previous_ai
    }
}

pub fn move_towards(id: usize, target_x: i32, target_y: i32, map: &Map, objects: &mut [Object]) {
    let dx = target_x - objects[id].x;
    let dy = target_y - objects[id].y;

    let distance = ((dx.pow(2) + dy.pow(2)) as f32).sqrt();

    let dx = (dx as f32 / distance).round() as i32;
    let dy = (dy as f32 / distance).round() as i32;

    move_by(id, dx, dy, map, objects);
}

pub fn move_by(id: usize, dx: i32, dy: i32, map: &Map, objects: &mut [Object]) {
    let (x, y) = objects[id].pos();
    let blocked = is_blocked(x + dx, y + dy, map, objects);
    if !blocked {
        objects[id].x += dx;
        objects[id].y += dy;
    }
}
