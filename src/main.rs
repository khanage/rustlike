#![allow(dead_code,unused_imports,unused_variables)]
extern crate tcod;
extern crate rand;
extern crate serde;
extern crate serde_json;

#[macro_use]
extern crate serde_derive;

mod types;
mod util;
mod consts;
mod inputs;
mod items;
mod monsters;
mod ai;
mod display;

use tcod::{
    console::*,
    colors::{self, Color},
    input::*,
    input::{self, Event, EventFlags, KeyCode::*, Mouse},
    map::{Map as FovMap, *},
};
use std::io::{Read, Write};
use std::fs::File;
use std::error::Error;
use std::cmp;
use rand::{distributions::{IndependentSample, Weighted, WeightedChoice}, Rng};

use consts::*;
use types::{DeathCallback, Fighter, Game, Map, Object, PlayerAction, Tcod, Tile};
use logging::*;
use items::*;
use ai::{Ai, ai_take_turn, move_by};
use util::*;

mod logging {
    use crate::types::*;

    pub trait MessageLog {
        fn gutter_text<T>(&mut self, message: T, color: Color)
        where T: Into<String>;
    }

    impl MessageLog for Vec<(String, Color)> {
        fn gutter_text<T>(& mut self, message: T, color: Color)
        where T: Into<String> {
            self.push((message.into(), color));
        }
    }

    impl MessageLog for Game {
        fn gutter_text<T>(&mut self, message: T, color: Color)
        where T: Into<String> {
            self.log.gutter_text(message, color);
        }
    }
}

fn level_up(objects: &mut [Object], game: &mut Game, tcod: &mut Tcod) {
    let player = &mut objects[PLAYER];
    let level_up_xp = LEVEL_UP_BASE + player.level * LEVEL_UP_FACTOR;

    if player.fighter.as_ref().map_or(0, |f| f.xp) >= level_up_xp {
        player.level += 1;
        game.gutter_text(
            format!(
                "You battle skills grow stronger. You have reached level {}.",
                player.level
            ),
            colors::YELLOW,
        );


        let fighter = player.fighter.as_mut().unwrap();
        let mut choice = None;

        while choice.is_none() {
            choice = display::menu(
                "Level up! Choose a stat to raise:\n",
                &[
                    format!("Constitution (+20 HP, from {})", fighter.base_max_hp),
                    format!("Strength (+1 attack, from {})", fighter.base_power),
                    format!("Agility (+1 defense, from {})", fighter.base_defense),
                ],
                LEVEL_SCREEN_WIDTH,
                &mut tcod.root,
            );
        }
        fighter.xp -= level_up_xp;
        match choice.unwrap() {
            0 => {
                fighter.base_max_hp += 20;
                fighter.hp += 20;
            }
            1 => {
                fighter.base_power += 1;
            }
            2 => {
                fighter.base_defense += 1;
            }
            _ => unreachable!(),
        }
    }
}

fn player_move_or_attack(dx: i32, dy: i32, game: &mut Game, objects: &mut [Object]) {
    let x = objects[PLAYER].x + dx;
    let y = objects[PLAYER].y + dy;

    let target_id = objects.
        iter().
        position(|object| object.fighter.is_some() && object.pos() == (x, y));

    match target_id {
        Some(target_id) => {
            let (player, target) = crate::util::mut_two(PLAYER, target_id, objects);
            player.attack(target, game);
        }
        None => {
            move_by(PLAYER, dx, dy, &game.map, objects);
        }
    }
}


fn is_blocked(x: i32, y: i32, map: &Map, objects: &[Object]) -> bool {
    if map[x as usize][y as usize].blocked {
        println!("BLOCKED BYT TILE");
        return true
    }
    objects.iter().any(|object| object.blocks && object.pos() == (x, y))
}

fn target_monster(
    tcod: &mut Tcod,
    objects: &mut [Object],
    game: &mut Game,
    max_range: Option<f32>,
) -> Option<usize> {
    loop {
        match target_tile(tcod, objects, game, max_range) {
            Some((x, y)) => {
                for (id, obj) in objects.iter().enumerate() {
                    if obj.pos() == (x, y) && obj.fighter.is_some() && id != PLAYER {
                        return Some(id)
                    }
                }
            },
            None => return None,
        }
    }
}

pub fn closest_monster(max_range: i32, objects: &mut [Object], tcod: &Tcod) -> Option<usize> {
    let mut closest_enemy = None;
    let mut closest_dist = (max_range + 1) as f32;

    for (id, object) in objects.iter().enumerate() {
        if (id != PLAYER)
            && object.fighter.is_some()
            && object.ai.is_some()
            && tcod.fov.is_in_fov(object.x, object.y)
        {
            let dist = objects[PLAYER].distance_to(object);
            if dist < closest_dist {
                closest_dist = dist;
                closest_enemy = Some(id);
            }
        }
    }

    closest_enemy
}

pub fn target_tile(
    tcod: &mut Tcod,
    objects: &mut [Object],
    game: &mut Game,
    max_range: Option<f32>,
) -> Option<(i32, i32)> {
    use tcod::input::KeyCode::Escape;

    loop {
        tcod.root.flush();

        level_up(objects, game, tcod);

        let event = input::check_for_event(input::KEY_PRESS | input::MOUSE).map (|e| e.1);

        let mut key = None;

        match event {
            Some(Event::Mouse(m)) => tcod.mouse = m,
            Some(Event::Key(k)) => key = Some(k),
            None => {},
        }

        display::render_all(tcod, objects, game, false);

        let (x, y) = (tcod.mouse.cx as i32, tcod.mouse.cy as i32);

        let in_fov = (x < MAP_WIDTH) && (y < MAP_HEIGHT) && tcod.fov.is_in_fov(x, y);
        let in_range = max_range.map_or(true, |range| objects[PLAYER].distance(x, y) <= range);

        if tcod.mouse.lbutton_pressed && in_fov && in_range {
            return Some((x, y));
        }

        let escape = key.map_or(false, |k| k.code == Escape);
        if tcod.mouse.rbutton_pressed || escape {
            return None;
        }
    }
}

fn make_map(objects: &mut Vec<Object>) -> Map {
    let mut map = vec![vec![Tile::wall(); MAP_HEIGHT as usize]; MAP_WIDTH as usize];
    assert_eq!(&objects[PLAYER] as *const _, &objects[0] as *const _);
    objects.truncate(1);

    let mut rooms = vec![];

    for _ in 0..MAX_ROOMS {
        let w = rand::thread_rng().gen_range(ROOM_MIN_SIZE, ROOM_MAX_SIZE + 1);
        let h = rand::thread_rng().gen_range(ROOM_MIN_SIZE, ROOM_MAX_SIZE + 1);

        let x = rand::thread_rng().gen_range(0, MAP_WIDTH - w);
        let y = rand::thread_rng().gen_range(0, MAP_HEIGHT - h);

        let new_room = Rect::new(x, y, w, h);

        let failed = rooms
            .iter()
            .any(|other_room| new_room.intersects_with(other_room));

        if !failed {
            create_room(new_room, &mut map);

            place_objects(new_room, objects, &mut map);

            let (new_x, new_y) = new_room.center();

            if !rooms.is_empty() {
                let (prev_x, prev_y) = rooms[rooms.len() - 1].center();

                if rand::random() {
                    create_h_tunnel(prev_x, new_x, prev_y, &mut map);
                    create_v_tunnel(prev_y, new_y, new_x, &mut map);
                } else {
                    create_v_tunnel(prev_y, new_y, prev_x, &mut map);
                    create_h_tunnel(prev_x, new_x, new_y, &mut map);
                }
            } else {
                objects[PLAYER].set_pos(new_x, new_y);
            }

            rooms.push(new_room);
        }
    }

    let (last_room_x, last_room_y) = rooms[rooms.len() - 1].center();

    let mut stairs = Object::new(
        last_room_x,
        last_room_y,
        '<',
        colors::WHITE,
        "stairs",
        false,
    );

    stairs.always_visible = true;
    objects.push(stairs);

    map
}

fn next_level(tcod: &mut Tcod, objects: &mut Vec<Object>, game: &mut Game) {
    game.log.gutter_text(
        "You take a moment to rest, and recover your strength.",
        colors::VIOLET,
    );

    let heal_hp = objects[PLAYER].max_hp(game) / 2;
    objects[PLAYER].heal(heal_hp, game);

    game.log.gutter_text(
        "After a rare moment of peace, you descend deeper into \
         the heart of the dungeon..",
        colors::RED,
    );

    game.dungeon_level += 1;
    game.map = make_map(objects);

    initialize_fov(&game.map, tcod);
}

#[derive(Clone,Copy,Debug)]
struct Rect {
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
}

impl Rect {
    pub fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Rect {
            x1: x,
            y1: y,
            x2: x + w,
            y2: y + h,
        }
    }

    pub fn center(&self) -> (i32, i32) {
        let center_x = (self.x1 + self.x2) / 2;
        let center_y = (self.y1 + self.y2) / 2;

        (center_x, center_y)
    }

    pub fn intersects_with(&self, other: &Rect) -> bool {
        (self.x1 <= other.x2)
            && (self.x2 >= other.x1)
            && (self.y1 <= other.y2)
            && (self.y2 >= other.y1)
    }
}

fn create_room(room: Rect, map: &mut Map) {
    for x in (room.x1 + 1)..room.x2 {
        for y in (room.y1 + 1)..room.y2 {
            map[x as usize][y as usize] = Tile::empty();
        }
    }
}

fn create_h_tunnel(x1: i32, x2: i32, y: i32, map: &mut Map) {
    for x in cmp::min(x1, x2)..(cmp::max(x1, x2) + 1) {
        map[x as usize][y as usize] = Tile::empty();
    }
}

fn create_v_tunnel(y1: i32, y2: i32, x: i32, map: &mut Map) {
    for y in cmp::min(y1, y2)..(cmp::max(y1, y2) + 1) {
        map[x as usize][y as usize] = Tile::empty();
    }
}

fn place_objects(room: Rect, objects: &mut Vec<Object>, map: &mut Map,) {
    let level = 1;
    let max_monsters = from_dungeon_level(
        &[
            Transition { level: 1, value: 2 },
            Transition { level: 4, value: 3 },
            Transition { level: 6, value: 5 },
        ],
        level,
    );
    let num_monsters = rand::thread_rng().gen_range(0, max_monsters + 1);
    let monsters = &mut monsters::monster_table_for_level(level);
    let monster_choice = WeightedChoice::new(monsters);

    for _ in 0..num_monsters {
        let x = rand::thread_rng().gen_range(room.x1 + 1, room.x2);
        let y = rand::thread_rng().gen_range(room.y1 + 1, room.y2);

        if !is_blocked(x, y, map, objects) {
            let monster = monsters::make_monster(x, y, monster_choice.ind_sample(&mut rand::thread_rng()));
            objects.push(monster);
        }
    }

    let max_items = from_dungeon_level(
        &[
            Transition { level: 1, value: 1, },
            Transition { level: 4, value: 2, },
        ],
        level,
    );

    let num_items = rand::thread_rng().gen_range(0, max_items + 1);

    for _ in 0..num_items {
        let x = rand::thread_rng().gen_range(room.x1 + 1, room.x2);
        let y = rand::thread_rng().gen_range(room.y1 + 1, room.y2);

        let items = &mut items::item_table_for_level(level);
        let item_choice = WeightedChoice::new(items);

        if !is_blocked(x, y, map, objects) {
            let dice = rand::random::<f32>();
            let item_type = item_choice.ind_sample(&mut rand::thread_rng());
            let item = items::make_item(x, y, item_type);

            objects.push(item);
        }
    }
}


fn new_game(tcod: &mut Tcod) -> (Vec<Object>, Game) {
    let mut player = Object::new(0, 0, '@', colors::WHITE, "player", true);
    player.alive = true;
    player.fighter = Some(Fighter {
        base_max_hp: 100,
        hp: 100,
        base_defense: 1,
        base_power: 2,
        on_death: DeathCallback::Player,
        xp: 0,
        base_movement: 4,
    });

    let mut objects = vec![player];

    let mut game = Game {
        map: make_map(&mut objects),
        log: vec![],
        inventory: vec![],
        dungeon_level: 1,
    };

    let mut dagger = Object::new(0, 0, '-', colors::SKY, "dagger", false);
    dagger.item = Some(Item::Sword);
    dagger.equipment = Some(Equipment {
        equipped: true,
        slot: Slot::LeftHand,
        max_hp_bonus: 0,
        defense_bonus: 0,
        power_bonus: 2,
    });

    game.inventory.push(dagger);

    initialize_fov(&game.map, tcod);

    game.log.gutter_text(
        "Welcome stranger! Prepare to perish in the Tombs of the Ancient Kings.",
        colors::WHITE,
    );

    (objects, game)
}

fn initialize_fov(map: &Map, tcod: &mut Tcod) {
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            tcod.fov.set(
                x,
                y,
                !map[x as usize][y as usize].block_sight,
                !map[x as usize][y as usize].blocked,
            );
        }
    }

    tcod.con.clear();
}

fn play_game(
    objects: &mut Vec<Object>,
    game: &mut Game,
    tcod: &mut Tcod,
) {
    let mut previous_player_position = (-1, -1);
    let mut key = Default::default();

    while !tcod.root.window_closed() {
        let player = &objects[PLAYER];
        let new_player_position = (player.x, player.y);
        let fov_recompute = previous_player_position != new_player_position;
        previous_player_position = new_player_position;

        match input::check_for_event(input::MOUSE | input::KEY_PRESS) {
            Some((_, Event::Mouse(m))) => tcod.mouse = m,
            Some((_, Event::Key(k))) => key = k,
            _ => key = Default::default(),
        }

        tcod.con.clear();
        display::render_all(
            tcod,
            &objects,
            game,
            fov_recompute,
        );
        tcod.root.flush();

        let player = &mut objects[PLAYER];

        let player_action = inputs::handle_keys(
            key,
            tcod,
            objects,
            game,
        );
        if player_action == PlayerAction::Exit {
            save_game(objects, game).expect("Failed to save!");
            break
        }

        if objects[PLAYER].alive && player_action != PlayerAction::DidntTakeTurn {
            for id in 0..objects.len() {
                if objects[id].ai.is_some() {
                    ai_take_turn(id, game, objects, &tcod.fov)
                }
            }
        }
    }
}

fn load_game() -> Result<(Vec<Object>, Game), Box<Error>> {
    let mut json_save_state = String::new();
    let mut file = File::open("savegame")?;
    file.read_to_string(&mut json_save_state)?;
    let result = serde_json::from_str::<(Vec<Object>, Game)>(&json_save_state)?;

    Ok(result)
}

fn main_menu(tcod: &mut Tcod) {
    let img = tcod::image::Image::from_file("menu_background.png")
        .ok()
        .expect("Background image not found");

    while !tcod.root.window_closed() {
        tcod::image::blit_2x(&img, (0, 0), (-1, -1), &mut tcod.root, (0, 0));

        tcod.root.set_default_foreground(colors::LIGHT_YELLOW);
        tcod.root.print_ex(
            SCREEN_WIDTH / 2,
            SCREEN_HEIGHT / 2 - 4,
            BackgroundFlag::None,
            TextAlignment::Center,
            "TOMBS OF THE ANCIENT KINGS",
        );
        tcod.root.print_ex(
            SCREEN_WIDTH / 2,
            SCREEN_HEIGHT - 2,
            BackgroundFlag::None,
            TextAlignment::Center,
            "KHANAGE Games"
        );

        let choices = &["Play a new game", "Continue last game", "Quit"];
        let choice = display::menu("", choices, 24, &mut tcod.root);

        match choice {
            Some(0) => {
                let (mut objects, mut game) = new_game(tcod);
                play_game(&mut objects, &mut game, tcod);
            }
            Some(1) => {
                match load_game() {
                    Ok((mut objects, mut game)) => {
                        initialize_fov(&game.map, tcod);
                        play_game(&mut objects, &mut game, tcod);
                    }
                    Err(_e) => {
                        display::msgbox("\nNo saved game to load.\n", 24, &mut tcod.root);
                        continue;
                    }
                }
            }
            Some(2) => {
                break;
            }
            _ => {}
        }
    }
}

fn save_game(objects: &[Object], game: &Game) -> Result<(), Box<Error>> {
    let save_data = serde_json::to_string(&(objects, game))?;
    let mut file = File::create("savegame")?;
    file.write_all(save_data.as_bytes())?;

    Ok(())
}

fn main() {
    let root = Root::initializer()
        .font("arial10x10.png", FontLayout::Tcod)
        .font_type(FontType::Greyscale)
        .size(SCREEN_WIDTH, SCREEN_HEIGHT)
        .title("Rust/libtcod tutorial")
        .init();
    tcod::system::set_fps(LIMIT_FPS);

    let mut tcod = Tcod {
        root: root,
        con: Offscreen::new(MAP_WIDTH, MAP_HEIGHT),
        panel: Offscreen::new(SCREEN_WIDTH, PANEL_HEIGHT),
        fov: FovMap::new(MAP_WIDTH, MAP_HEIGHT),
        mouse: Default::default(),
    };

    main_menu(&mut tcod);
}
