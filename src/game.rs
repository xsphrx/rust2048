use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use itertools::Itertools;
use rand::seq::SliceRandom;
use std::cell::RefCell;
use std::collections::HashMap;
use std::mem;
use std::{
    error::Error,
    io,
    time::{Duration, Instant},
};
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Span, Spans},
    widgets::{
        canvas::{Canvas, Label, Line, Map, MapResolution, Rectangle},
        Block, BorderType, Borders, Cell, LineGauge, Paragraph, Row, Table, Wrap,
    },
    Frame, Terminal,
};

pub const MARGINX: u16 = 2;
pub const MARGINY: u16 = 1;

#[derive(Clone, Copy, PartialEq)]
pub enum Move {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Eq, Hash)]
pub struct Position {
    pub x: u16,
    pub y: u16,
}

impl Position {
    pub fn new(x: u16, y: u16) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Coordinates {
    pub x: u16,
    pub y: u16,
}

impl Coordinates {
    pub fn new(x: u16, y: u16) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Tile {
    pub coordinates: Coordinates,
    pub n: u32,
}

impl Tile {
    pub fn new(coordinates: Coordinates, n: u32) -> Self {
        Tile { coordinates, n }
    }

    pub fn mv(&mut self, coordinates: Coordinates) {
        self.coordinates = coordinates
    }
    pub fn update_n(&mut self, n: u32) {
        self.n = n;
    }
}

#[derive(Debug, PartialEq)]
pub struct Grid {
    pub tiles: HashMap<Position, Tile>,
    pub moving_tiles: Vec<(Position, Position)>,
    pub size: u16,
    pub tile_width: u16,
    pub tile_height: u16,
    pub coordinates: Coordinates,
}

pub enum Flip {
    Horizontal,
    Clock,
    CounterClock,
}

impl Grid {
    pub fn new(tile_size: u16, coordinates: Coordinates, size: u16) -> Self {
        let tile_width = tile_size;
        let tile_height = tile_size / 2;

        Self {
            tiles: HashMap::new(),
            moving_tiles: vec![],
            size,
            tile_width,
            tile_height,
            coordinates,
        }
    }

    pub fn width(&self) -> u16 {
        2 + self.tile_width * 4 + MARGINX * 4
    }

    pub fn height(&self) -> u16 {
        self.width() / 2
    }

    pub fn get_tile_mut(&mut self, pos: Position) -> Option<&mut Tile> {
        if let Some(_) = self.tiles.get(&pos) {
            Some(self.tiles.get_mut(&pos).unwrap())
        } else {
            None
        }
    }

    pub fn get_tile(&mut self, pos: Position) -> Option<Tile> {
        if let Some(tile) = self.tiles.get(&pos) {
            Some(*tile)
        } else {
            None
        }
    }

    pub fn get_coordinates_at(&self, pos: Position) -> Coordinates {
        Coordinates {
            x: self.coordinates.x + MARGINX + pos.x * MARGINX + pos.x * self.tile_width,
            y: self.coordinates.y + MARGINY + pos.y * MARGINY + pos.y * self.tile_height,
        }
    }
    pub fn insert_tile(&mut self, pos: Position, n: u32) {
        self.tiles
            .insert(pos, Tile::new(self.get_coordinates_at(pos), n));
    }

    pub fn remove_tile(&mut self, pos: Position) {
        self.tiles.remove(&pos);
    }

    pub fn remove_moving_tile(&mut self, pos: Position) {
        let index = self
            .moving_tiles
            .iter()
            .position(|((p, _))| p == &pos)
            .unwrap();
        self.moving_tiles.remove(index);
    }

    pub fn spawn_random_tile(&mut self) -> Result<(), String> {
        let mut available = vec![];
        for x in 0..self.size {
            for y in 0..self.size {
                if !self.tiles.contains_key(&Position::new(x, y)) {
                    available.push((x, y));
                }
            }
        }
        if available.len() < 1 {
            return Err("No space left".to_string());
        }

        if let Some((x, y)) = available.choose(&mut rand::thread_rng()) {
            self.insert_tile(Position::new(*x, *y), 2);
            return Ok(());
        }

        Err("Something went wrong".to_string())
    }

    pub fn flip(&mut self, flip: Flip) {
        let s = self.size - 1;
        self.moving_tiles = self
            .moving_tiles
            .iter()
            .map(|(pos, new_pos)| match flip {
                Flip::Horizontal => (
                    Position::new(s - pos.x, pos.y),
                    Position::new(s - new_pos.x, new_pos.y),
                ),
                Flip::CounterClock => (
                    Position::new(s - pos.y, pos.x),
                    Position::new(s - new_pos.y, new_pos.x),
                ),
                Flip::Clock => (
                    Position::new(pos.y, s - pos.x),
                    Position::new(new_pos.y, s - new_pos.x),
                ),
            })
            .collect();
        self.tiles = self
            .tiles
            .iter()
            .map(|(pos, tile)| match flip {
                Flip::Horizontal => (Position::new(s - pos.x, pos.y), *tile),
                Flip::CounterClock => (Position::new(s - pos.y, pos.x), *tile),
                Flip::Clock => (Position::new(pos.y, s - pos.x), *tile),
            })
            .collect();
    }

    fn get_desired_position(
        &mut self,
        pos: Position,
        n: u32,
        unavailable: &Vec<Position>,
    ) -> (Position, u32) {
        let Position { x, y } = pos;
        if x == 0_u16 {
            return (Position::new(x, y), n);
        }

        let mut new_x = x;
        for checking_x in (0..x).rev() {
            let new_pos = Position::new(checking_x, y);
            if unavailable.contains(&new_pos) {
                break;
            }

            if let Some(checking_tile) = self.get_tile(new_pos) {
                if checking_tile.n == n {
                    return (Position::new(checking_x, y), n * 2);
                } else {
                    break;
                }
            } else {
                new_x = checking_x;
            }
        }
        (Position::new(new_x, y), n)
    }

    pub fn check(&mut self, mv: Move) -> Vec<(Position, Position)> {
        let mut new_grid = Grid {
            tiles: HashMap::new(),
            moving_tiles: vec![],
            ..*self
        };

        match mv {
            Move::Right => {
                self.flip(Flip::Horizontal);
            }
            Move::Up => {
                self.flip(Flip::Clock);
            }
            Move::Down => {
                self.flip(Flip::CounterClock);
            }
            _ => (),
        };

        let mut unavailable = vec![];
        for (pos, tile) in self.tiles.iter().sorted_by_key(|(p, _)| p.x) {
            let (new_pos, n) =
                new_grid.get_desired_position(Position::new(pos.x, pos.y), tile.n, &unavailable);
            if n > tile.n {
                unavailable.push(new_pos);
            }
            new_grid.insert_tile(new_pos, n);
            if pos != &new_pos {
                new_grid.moving_tiles.push((*pos, new_pos));
            }
        }

        match mv {
            Move::Right => {
                self.flip(Flip::Horizontal);
                new_grid.flip(Flip::Horizontal);
            }
            Move::Up => {
                self.flip(Flip::CounterClock);
                new_grid.flip(Flip::CounterClock);
            }
            Move::Down => {
                self.flip(Flip::Clock);
                new_grid.flip(Flip::Clock);
            }
            _ => (),
        };

        new_grid.moving_tiles
    }

    pub fn on_tick(&mut self, mv: Option<Move>, _animating: &mut bool) {
        if self.moving_tiles.len() > 0 {
            for (pos, new_pos) in self.moving_tiles.clone().iter() {
                let desired = self.get_coordinates_at(*new_pos);
                let tile = self.get_tile(*pos).unwrap();
                let current = tile.coordinates;

                let mut x = current.x;
                let mut y = current.y;

                match desired {
                    _ if desired.x > current.x => x += 4,
                    _ if desired.x < current.x => x -= 4,
                    _ if desired.y > current.y => y += 2,
                    _ if desired.y < current.y => y -= 2,
                    _ => {}
                }

                if desired == Coordinates::new(x, y) {
                    if let Some(tile) = self.get_tile(*new_pos) {
                        self.insert_tile(*new_pos, tile.n * 2);
                    } else {
                        let n = self.get_tile(*pos).unwrap().n;
                        self.insert_tile(*new_pos, n);
                    }
                    self.remove_tile(*pos);
                    self.remove_moving_tile(*pos);
                } else {
                    let tile = self.get_tile_mut(*pos).unwrap();
                    tile.mv(Coordinates::new(x, y));
                }
            }

            if self.moving_tiles.len() == 0 {
                self.spawn_random_tile();
            }

            return;
        }

        // loop through tiles
        if let None = mv {
            return;
        }

        self.moving_tiles = self.check(mv.unwrap());

        // if *self != new_grid {
        //     self.tiles = new_grid.tiles;
        //     let _ = self.spawn_random_tile();
        // }
    }
}

pub fn print_grid(grid: &Grid) {
    for (pos, tile) in grid.tiles.iter().sorted_by_key(|(pos, _)| pos.x) {
        println!(
            "({}, {}): {}  ({}, {})",
            pos.x, pos.y, tile.n, tile.coordinates.x, tile.coordinates.y
        );
    }
    println!();
    for (pos, new_pos) in grid.moving_tiles.iter().sorted_by_key(|(pos, _)| pos.x) {
        println!("({}, {}): ({}, {})", pos.x, pos.y, new_pos.x, new_pos.y);
    }
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;
    type P = Position;
    type C = Coordinates;

    #[test]
    fn get_tile_position() {
        let tile_size = 10;
        let grid = Grid::new(tile_size, C::new(0, 0), 4);
        assert!(grid.get_coordinates_at(P::new(0, 0)) == C::new(MARGINX, MARGINY));
        assert!(
            grid.get_coordinates_at(P::new(1, 1))
                == C::new(2 * MARGINX + tile_size, 2 * MARGINY + tile_size / 2)
        );
        assert!(
            grid.get_coordinates_at(P::new(0, 2))
                == C::new(MARGINX, 3 * MARGINY + (tile_size / 2 * 2))
        );
    }

    // #[test]
    // fn getting_new_xy() {
    //     let mut grid = Grid::new(10, C::new(0, 0), 4);
    //     assert!(grid.get_desired_position(P::new(0, 1), 2) == (P::new(0, 1), 2));
    //     assert!(grid.get_desired_position(P::new(1, 1), 2) == (P::new(0, 1), 2));
    //     assert!(grid.get_desired_position(P::new(2, 0), 4) == (P::new(0, 0), 4));
    //     grid.insert_tile(P::new(0, 1), 2);
    //     dbg!(&grid);
    //     assert!(grid.get_desired_position(P::new(1, 1), 2) == (P::new(0, 1), 4));
    //     grid.insert_tile(P::new(0, 1), 4);
    //     assert!(grid.get_desired_position(P::new(1, 1), 4) == (P::new(0, 1), 8));
    //     assert!(grid.get_desired_position(P::new(1, 1), 2) == (P::new(1, 1), 2));
    //     assert!(grid.get_desired_position(P::new(3, 1), 4) == (P::new(0, 1), 8));
    //     grid.insert_tile(P::new(1, 1), 2);
    //     assert!(grid.get_desired_position(P::new(3, 1), 2) == (P::new(1, 1), 4));
    // }

    #[test]
    fn one_tile_move_right() {
        let mut grid = Grid::new(10, C::new(0, 0), 4);
        grid.insert_tile(P::new(2, 0), 2);
        grid.insert_tile(P::new(0, 0), 2);
        grid.on_tick(Some(Move::Left), &mut false);
        grid.on_tick(None, &mut false);
        grid.on_tick(None, &mut false);
        grid.on_tick(None, &mut false);
        grid.on_tick(None, &mut false);
        grid.on_tick(None, &mut false);
        grid.on_tick(None, &mut false);
        grid.on_tick(None, &mut false);
        grid.on_tick(None, &mut false);
        grid.on_tick(None, &mut false);
        grid.on_tick(None, &mut false);
        grid.on_tick(None, &mut false);
        grid.on_tick(None, &mut false);
        grid.on_tick(None, &mut false);
        grid.on_tick(None, &mut false);
        grid.on_tick(None, &mut false);
        grid.on_tick(None, &mut false);
        grid.on_tick(None, &mut false);
        grid.on_tick(None, &mut false);
        grid.on_tick(None, &mut false);
        grid.on_tick(None, &mut false);
        grid.on_tick(None, &mut false);
        grid.on_tick(None, &mut false);
        grid.on_tick(None, &mut false);
        grid.on_tick(None, &mut false);
        grid.on_tick(None, &mut false);
        grid.on_tick(None, &mut false);
        grid.on_tick(None, &mut false);
        grid.on_tick(None, &mut false);
        grid.on_tick(None, &mut false);
        grid.on_tick(None, &mut false);
        print_grid(&grid);
        // assert!(grid.get_tile(0, 0).unwrap().n == 4);
        // grid.insert_tile(3, 0, 4);
        // grid.on_tick(Some(Move::Right), &mut false);
        // dbg!(&grid);
        //
        assert!(false);
    }

    // #[test]
    // fn two_tiles_move_left() {
    //     let mut game = Game::new(10, 0, 0, 4);
    //     let position = game.get_tile_position(1, 1);
    //     game.grid.insert((1, 1), Tile::new(position, 2));
    //     game.grid.insert((2, 1), Tile::new(position, 2));
    //     game.on_tick(Some(Move::Left), &mut false);

    //     let mut tmp_game = Game::new(10, 0, 0, 4);
    //     let position = tmp_game.get_tile_position(0, 1);
    //     tmp_game.grid.insert((0, 1), Tile::new(position, 4));
    //     dbg!(&game);
    //     assert!(game == tmp_game);
    // }
}
