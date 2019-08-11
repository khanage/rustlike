pub use tcod::colors::Color;

use tcod::{
    colors,
    console::{BackgroundFlag, Console, Root, Offscreen},
    map::{Map as FovMap},
    input::{Mouse},
};

use rand::Rng;

use crate::{
    consts::*,
    util::*,
    logging::MessageLog,
    items::{Item, Equipment},
    ai::Ai,
    closest_monster,
    target_tile,
};

pub type Map = Vec<Vec<Tile>>;
pub type Messages = Vec<(String, Color)>;

#[derive(Debug, Deserialize, Serialize)]
pub struct Object {
    pub name: String,

    pub level: i32,

    pub x: i32,
    pub y: i32,

    pub char: char,
    pub color: Color,

    pub blocks: bool,
    pub alive: bool,

    pub fighter: Option<Fighter>,
    pub ai: Option<Ai>,

    pub item: Option<Item>,

    pub always_visible: bool,

    pub equipment: Option<Equipment>,
}

impl Object {
    pub fn new(x: i32, y: i32, char: char, color: Color, name: &str, blocks: bool) -> Self {
        Object {
            x: x,
            y: y,
            level: 1,
            char: char,
            color: color,
            name: name.to_string(),
            blocks: blocks,
            alive: false,
            fighter: Option::None,
            ai: Option::None,
            item: Option::None,
            always_visible: false,
            equipment: None,
        }
    }

    pub fn draw(&self, con: &mut Console) {
        con.set_default_foreground(self.color);
        con.put_char(self.x, self.y, self.char, BackgroundFlag::None);
    }

    pub fn pos(&self) -> (i32, i32) {
        (self.x, self.y)
    }

    pub fn set_pos(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
    }

    pub fn distance_to(&self, other: &Object) -> f32 {
        let dx = other.x - self.x;
        let dy = other.y - self.y;

        ((dx.pow(2) + dy.pow(2)) as f32).sqrt()
    }

    pub fn distance(&self, x: i32, y: i32) -> f32 {
        (((x - self.x).pow(2) + (y - self.y).pow(2)) as f32).sqrt()
    }

    pub fn take_damage(&mut self, damage: i32, game: &mut Game) -> Option<i32> {
        if let Some(fighter) = self.fighter.as_mut() {
            if damage > 0 {
                fighter.hp -= damage;
            }
        }

        if let Some(fighter) = self.fighter {
            if fighter.hp <= 0 {
                self.alive = false;
                fighter.on_death.callback(self, game);
                return Some(fighter.xp)
            }
        }

        None
    }

    pub fn attack(
        &mut self,
        target: &mut Object,
        game: &mut Game,
    ) {
        let power = self.power(game);
        let defense = target.defense(game);

        let damage = power - defense;

        if damage > 0 {
            game.log.gutter_text(
                format!("{} attacks {} for {} hit points.", self.name, target.name, damage),
                colors::WHITE,
            );
            if let Some(xp) = target.take_damage(damage, game) {
                self.fighter.as_mut().unwrap().xp += xp;
            }
        } else {
            game.log.gutter_text(
                format!(
                    "{} attacks {} but it has no effect!",
                    self.name, target.name
                ),
                colors::WHITE,
            );
        }
    }

    pub fn heal(&mut self, amount: i32, game: &Game) {
        let max_hp = self.max_hp(game);
        if let Some(ref mut fighter) = self.fighter {
            fighter.hp += amount;

            if fighter.hp > max_hp {
                fighter.hp = max_hp;
            }
        }
    }

    pub fn equip(&mut self, log: &mut Vec<(String,Color)>) {
        if self.item.is_none() {
            log.gutter_text(
                format!("Can't equip {:?} because it's not an Item.", self),
                colors::RED,
            );

            return;
        };

        if let Some(ref mut equipment) = self.equipment {
            if !equipment.equipped {
                equipment.equipped = true;
                log.gutter_text(
                    format!("Equipped {} on {}.", self.name, equipment.slot),
                    colors::LIGHT_GREEN,
                );
            }
        } else {
            log.gutter_text(
                format!("Can't equip {:?} because it's not an Equipment.", self),
                colors::RED,
            );
        }
    }

    pub fn dequip(&mut self, log: &mut Vec<(String, Color)>) {
        if self.item.is_none() {
            log.gutter_text(
                format!("Can't dequip {:?} because it's not an Item.", self),
                colors::RED,
            );
            return;
        };

        if let Some(ref mut equipment) = self.equipment {
            if equipment.equipped {
                equipment.equipped = false;
                log.gutter_text(
                    format!("Dequipped {} from {}", self.name, equipment.slot),
                    colors::LIGHT_YELLOW,
                );
            }
        } else {
            log.gutter_text(
                format!("Can't dequip {:?} because it's not an Equipment.", self),
                colors::RED,
            );
        }
    }

    pub fn get_all_equipped(&self, game: &Game) -> Vec<Equipment> {
        if self.name == "player" {
            game.inventory
                .iter()
                .filter(|item| item.equipment.map_or(false, |e| e.equipped))
                .map(|item| item.equipment.unwrap())
                .collect()
        } else {
            vec![]
        }
    }

    pub fn power(&self, game: &Game) -> i32 {
        let base_power = self.fighter.map_or(0, |f| f.base_power);
        let bonus: i32 = self
            .get_all_equipped(game)
            .iter()
            .map(|e| e.power_bonus)
            .sum();

        base_power + bonus
    }

    pub fn defense(&self, game: &Game) -> i32 {
        let base_defense = self.fighter.map_or(0, |f| f.base_defense);
        let bonus: i32 = self
            .get_all_equipped(game)
            .iter()
            .map(|e| e.defense_bonus)
            .sum();

        base_defense + bonus
    }

    pub fn max_hp(&self, game: &Game) -> i32 {


        let base_max_hp = self.fighter.map_or(0, |f| f.base_max_hp);
        let bonus: i32 = self
            .get_all_equipped(game)
            .iter()
            .map(|e| e.defense_bonus)
            .sum();

        base_max_hp + bonus
    }
}

#[derive(Deserialize,Serialize)]
pub struct Game {
    pub map: Map,
    pub log: Messages,
    pub inventory: Vec<Object>,
    pub dungeon_level: u32,
}

pub struct Tcod {
    pub root: Root,
    pub con: Offscreen,
    pub panel: Offscreen,
    pub fov: FovMap,
    pub mouse: Mouse,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlayerAction {
    TookTurn,
    DidntTakeTurn,
    Exit,
}

#[derive(Clone,Copy,Debug,Deserialize,Serialize)]
pub struct Tile {
    pub blocked: bool,
    pub block_sight: bool,
    pub explored: bool,
}

impl Tile {
    pub fn empty() -> Self {
        Tile {
            blocked: false,
            block_sight: false,
            explored: false,
        }
    }

    pub fn wall() -> Self {
        Tile {
            blocked: true,
            block_sight: true,
            explored: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct Fighter {
    pub hp: i32,

    pub base_movement: i32,
    pub base_max_hp: i32,
    pub base_defense: i32,
    pub base_power: i32,
    pub on_death: DeathCallback,

    pub xp: i32,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub enum DeathCallback {
    Player,
    Monster,
}

impl DeathCallback {
    fn callback(self, object: &mut Object, game: &mut Game) {
        use DeathCallback::*;

        let callback: fn(&mut Object, &mut Game) = match self {
            Player => player_death,
            Monster => monster_death,
        };

        callback(object, game);
    }
}

pub fn player_death(player: &mut Object, game: &mut Game) {
    game.log.gutter_text(
        "You died!",
        colors::RED,
    );

    player.char = '%';
    player.color = colors::DARK_RED;
}

pub fn monster_death(monster: &mut Object, game: &mut Game) {
    game.log.gutter_text(
        format!("{} is dead! You gain {} experience points.", monster.name, monster.fighter.unwrap().xp),
        colors::ORANGE,
    );

    monster.char = '%';
    monster.color = colors::DARK_RED;
    monster.blocks = false;
    monster.fighter = None;
    monster.ai = None;
    monster.name = format!("remains of {}", monster.name);
}
