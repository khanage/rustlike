use crate::{
    consts::*,
    types::*,
    logging::*,
    PlayerAction::{self, *},
    display::*,
    items::*,
    next_level,
    player_move_or_attack,
};

use tcod::{
    colors,
    input::{Key, Mouse, KeyCode::*},
    map::{Map as FovMap},
};

pub fn handle_keys(
    key: Key,
    tcod: &mut Tcod,
    objects: &mut Vec<Object>,
    game: &mut Game,
) -> PlayerAction {

    let player_alive = objects[PLAYER].alive;

    match (key, player_alive) {
        // Get a lot of this for some reason
        (Key { code: NoKey, .. }, ..) => DidntTakeTurn,

        (Key {
            code: Enter,
            alt: true,
            ..
        },
         _,
        ) => {
            let fullscreen = tcod.root.is_fullscreen();
            tcod.root.set_fullscreen(!fullscreen);
            DidntTakeTurn
        },

        (Key { code: Escape, .. }, _) => Exit,

        // Pressed '<'
        (Key { printable: ',', shift: true, .. }, true) => {
            let player_on_stairs = objects
                .iter()
                .any(|object| object.pos() == objects[PLAYER].pos() && object.name == "stairs");

            if player_on_stairs {
                next_level(tcod, objects, game);
            } else {
                game.log.gutter_text(
                    "You cannot go down from here",
                    colors::WHITE,
                );
            }

            DidntTakeTurn
        }

        (Key { code, printable, .. }, true) => match (code, printable) {
            (Char, '.') => EndedMove,

            (Char, 'k') | (Up, _) => Move(0, -1),
            (Char, 'j') | (Down, _) => Move(0, 1),
            (Char, 'h') | (Left, _) => Move(-1, 0),
            (Char, 'l') | (Right, _) => Move(1, 0),
            (Char, 'y') => Move(-1, -1),
            (Char, 'u') => Move(1, -1),
            (Char, 'n') => Move(-1, 1),
            (Char, 'm') => Move(1, 1),

            (Char, 'g') => {
                game.log.gutter_text(
                    format!("Picking up..."),
                    colors::WHITE,
                );

                let item_id = objects
                    .iter()
                    .position(|object| object.pos() == objects[PLAYER].pos() && object.item.is_some());

                println!("Item_id: {:?}", item_id);

                if let Some(item_id) = item_id {
                    pick_item_up(item_id as usize, objects, game);
                }

                DidntTakeTurn
            },

            (Char, 'd') => {
                let inventory_index = inventory_menu(
                    &mut game.inventory,
                    "Select the ittem to drop.\n",
                    &mut tcod.root,
                );

                if let Some(inventory_index) = inventory_index {
                    drop_item(inventory_index, game, objects);
                }

                DidntTakeTurn
            },

            (Char, 'i') => {
                let inventory_index = inventory_menu(
                    &game.inventory,
                    "Press they key next to an item to use it, or any other to cancel.\n",
                    &mut tcod.root,
                );
                if let Some(inventory_index) = inventory_index {
                    use_item(inventory_index, objects, game, tcod);
                }
                DidntTakeTurn
            },

            (Char, 'c') => {
                let player = &objects[PLAYER];
                let level = player.level;
                let level_up_xp = LEVEL_UP_BASE + player.level * LEVEL_UP_FACTOR;

                if let Some(fighter) = player.fighter.as_ref() {
                    let msg = format!(
                        "Character information

Level: {}
Experience: {}
Experience to level up: {}

Maximum HP: {}
Attack: {}
Defence: {}",
                        level,
                        fighter.xp,
                        level_up_xp,
                        player.max_hp(game),
                        player.power(game),
                        player.defense(game)
                    );

                    msgbox(&msg, CHARACTER_SCREEN_WIDTH, &mut tcod.root);
                }

                DidntTakeTurn
            }

            keycode => {
                println!("Key pressed: {:?}", keycode);
                DidntTakeTurn
            },
        },

        key => {
            println!("Got key: {:?}", key);
            DidntTakeTurn
        },
    }
}

pub fn get_names_under_mouse(
    mouse: Mouse,
    objects: &[Object],
    fov_map: &FovMap,
) -> String {
    let (x, y) = (mouse.cx as i32, mouse.cy as i32);

    let names = objects
        .iter()
        .filter(|obj| { obj.pos() == (x, y) && fov_map.is_in_fov(obj.x, obj.y) })
        .map(|obj| obj.name.clone())
        .collect::<Vec<_>>();

    names.join(", ")
}
