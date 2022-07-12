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

use super::drawing::{draw_number, draw_shape, Direction};

pub const MARGINX: u16 = 2;
pub const MARGINY: u16 = 1;

#[derive(Clone, Copy, PartialEq)]
pub enum Move {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq, Default)]
pub struct Coordinates {
    pub x: u16,
    pub y: u16,
}

impl Coordinates {
    pub fn new(x: u16, y: u16) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq, Default)]
pub struct Position {
    pub x: u16,
    pub y: u16,
}

impl Position {
    pub fn new(x: u16, y: u16) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tile {
    pub position: Position,
    pub desired_position: Position,
    pub coordinates: Coordinates,
    pub n: u32,
}

impl Tile {
    pub fn new(
        position: Position,
        desired_position: Position,
        coordinates: Coordinates,
        n: u32,
    ) -> Self {
        Self {
            position,
            desired_position,
            coordinates,
            n,
        }
    }

    pub fn update_n(&mut self, n: u32) {
        self.n = n;
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Grid {
    pub tiles: Vec<Tile>,
    pub size: u16,
    pub tile_width: u16,
    pub tile_height: u16,
    pub coordinates: Coordinates,
}

impl Grid {
    pub fn new(tile_size: u16, coordinates: Coordinates, size: u16) -> Self {
        let tile_width = tile_size;
        let tile_height = tile_size / 2;

        Grid {
            tiles: vec![],
            size,
            tile_width,
            tile_height,
            coordinates,
        }
    }

    pub fn get_tile(&self, position: Position) -> Option<&Tile> {
        self.tiles.iter().find(|t| t.position == position)
    }

    pub fn get_coordinates_at(&self, position: Position) -> Coordinates {
        let Position { x, y } = position;
        Coordinates {
            x: self.coordinates.x + MARGINX + x * MARGINX + x * self.tile_width,
            y: self.coordinates.y + MARGINY + y * MARGINY + y * self.tile_height,
        }
    }

    pub fn insert_tile(&mut self, position: Position, desired: Position, n: u32) {
        if let Some(_) = self.get_tile(position) {
            panic!("Tile at this position already exists!")
        }

        let tile = Tile::new(position, desired, self.get_coordinates_at(position), n);
        self.tiles.push(tile);
    }

    pub fn width(&self) -> u16 {
        2 + self.tile_width * 4 + MARGINX * 4
    }

    pub fn height(&self) -> u16 {
        self.width() / 2
    }

    pub fn spawn_random_tile(&mut self) -> Result<(), String> {
        let mut available = vec![];
        for x in 0..self.size {
            for y in 0..self.size {
                if let Some(_) = self.get_tile(Position::new(x, y)) {
                    available.push((x, y));
                }
            }
        }
        if available.len() < 1 {
            return Err("No space left".to_string());
        }

        if let Some((x, y)) = available.choose(&mut rand::thread_rng()) {
            let position = Position::new(*x, *y);
            self.insert_tile(position, position, 2);
            return Ok(());
        }

        Err("Something went wrong".to_string())
    }

    pub fn flip_horizontal(&mut self) -> Vec<Tile> {
        let s = self.size - 1;
        self.tiles
            .iter()
            .map(|t| {
                let pos = Position::new(s - t.position.x, t.position.y);
                let desired = Position::new(s - t.desired_position.x, t.desired_position.y);
                Tile::new(pos, desired, t.coordinates, t.n)
            })
            .collect()
    }

    pub fn flip_clock(&mut self) -> Vec<Tile> {
        let s = self.size - 1;
        self.tiles
            .iter()
            .map(|t| {
                let pos = Position::new(s - t.position.y, t.position.x);
                let desired = Position::new(s - t.desired_position.y, t.desired_position.x);
                Tile::new(pos, desired, t.coordinates, t.n)
            })
            .collect()
    }

    pub fn flip_counter_clock(&mut self) -> Vec<Tile> {
        let s = self.size - 1;
        self.tiles
            .iter()
            .map(|t| {
                let pos = Position::new(t.position.y, s - t.position.x);
                let desired = Position::new(t.desired_position.y, s - t.desired_position.x);
                Tile::new(pos, desired, t.coordinates, t.n)
            })
            .collect()
    }

    fn get_desired_position(&mut self, position: Position, n: u32) -> Position {
        let Position { x, y } = position;
        if x == 0_u16 {
            return Position::new(x, y);
        }

        let mut new_x = x;
        for checking_x in (0..x).rev() {
            if let Some(checking_tile) = &mut self.get_tile(Position::new(checking_x, y)) {
                if checking_tile.n == n {
                    return Position::new(checking_x, y);
                } else {
                    break;
                }
            } else {
                new_x = checking_x;
            }
        }
        Position::new(new_x, y)
    }

    pub fn check(&mut self, mv: Move) -> Grid {
        let mut new_grid = Grid {
            tiles: vec![],
            ..*self
        };
        match mv {
            Move::Left => {
                for Tile { position, n, .. } in
                    self.clone().tiles.iter().sorted_by_key(|t| t.position.x)
                {
                    let desired = new_grid.get_desired_position(*position, *n);
                    new_grid.insert_tile(*position, desired, *n);
                }
            }
            Move::Right => {
                for Tile { position, n, .. } in self
                    .clone()
                    .flip_horizontal()
                    .iter()
                    .sorted_by_key(|t| t.position.x)
                {
                    let desired = new_grid.get_desired_position(*position, *n);
                    new_grid.insert_tile(*position, desired, *n);
                }
                new_grid.tiles = new_grid.flip_horizontal();
            }
            Move::Up => {
                for Tile { position, n, .. } in self
                    .flip_counter_clock()
                    .clone()
                    .iter()
                    .sorted_by_key(|x| x.position.x)
                {
                    let desired = new_grid.get_desired_position(*position, *n);
                    new_grid.insert_tile(*position, desired, *n);
                }
                new_grid.tiles = new_grid.flip_clock();
            }
            Move::Down => {
                for Tile { position, n, .. } in self
                    .flip_clock()
                    .clone()
                    .iter()
                    .sorted_by_key(|x| x.position.x)
                {
                    let desired = new_grid.get_desired_position(*position, *n);
                    new_grid.insert_tile(*position, desired, *n);
                }
                new_grid.tiles = new_grid.flip_counter_clock();
            }
            _ => (),
        }

        new_grid
    }

    pub fn on_tick(&mut self, mv: Option<Move>, _animating: &mut bool) {
        if let None = mv {
            return;
        }

        // iterate through tiles and push them to the desired position
        self.tiles = self
            .tiles
            .iter()
            .map(|t| Tile {
                coordinates: self.get_coordinates_at(t.desired_position),
                position: t.desired_position,
                ..*t
            })
            .collect();
        // for tile in &mut self.tiles {
        //     tile.coordinates = self.get_coordinates_at(tile.desired_position);
        //     // if tile.position != tile.desired_position {
        //     //     let desired_coordinates = self.get_coordinates_at(tile.desired_position);
        //     //     if desired_coordinates == tile.coordinates {
        //     //         if let Some(checking_tile) = self.get_tile(tile.position) {

        //     //         }

        //     //     }
        //     // }
        // }

        let new_grid: Grid = self.check(mv.unwrap());

        if *self != new_grid {
            self.tiles = new_grid.tiles;
            // let _ = self.spawn_random_tile();
        }
    }

    pub fn render<B>(&self, f: &mut Frame<B>)
    where
        B: Backend,
    {
        // render the grid
        let rect = Rect {
            x: self.coordinates.x,
            y: self.coordinates.y,
            width: self.width(),
            height: self.height(),
        };
        let block = Block::default()
            .title("2048")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded);
        f.render_widget(block, rect);
        for x in 0..self.size {
            for y in 0..self.size {
                let Coordinates { x, y } = self.get_coordinates_at(Position::new(x, y));
                let rect = Rect {
                    x,
                    y,
                    width: self.tile_width,
                    height: self.tile_height,
                };
                let empty_tile = Block::default().style(Style::default().bg(Color::DarkGray));
                f.render_widget(empty_tile, rect);
            }
        }
        // render tiles
        for tile in self.tiles.iter() {
            let rect = Rect {
                x: tile.coordinates.x,
                y: tile.coordinates.y,
                width: self.tile_width,
                height: self.tile_height,
            };
            let canvas = Canvas::default()
                .marker(symbols::Marker::Braille)
                .x_bounds([0.0, 10.0])
                .y_bounds([0.0, 10.0])
                .paint(|ctx| {
                    draw_number(ctx, tile.n);
                });
            f.render_widget(canvas, rect);
            let tile = Block::default().style(Style::default().bg(Color::DarkGray));
            f.render_widget(tile, rect);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_tile_position() {
        let tile_size = 10;
        let grid = Grid::new(tile_size, Coordinates::new(0, 0), 4);
        assert!(grid.get_coordinates_at(Position::new(0, 0)) == Coordinates::new(MARGINX, MARGINY));
        assert!(
            grid.get_coordinates_at(Position::new(1, 1))
                == Coordinates::new(2 * MARGINX + tile_size, 2 * MARGINY + tile_size / 2)
        );
        assert!(
            grid.get_coordinates_at(Position::new(0, 2))
                == Coordinates::new(MARGINX, 3 * MARGINY + (tile_size / 2 * 2))
        );
    }

    #[test]
    fn getting_new_xy() {
        let mut grid = Grid::new(10, Coordinates::new(0, 0), 4);
        assert!(grid.get_desired_position(Position::new(0, 1), 2) == Position::new(0, 1));
        assert!(grid.get_desired_position(Position::new(1, 1), 2) == Position::new(0, 1));
        assert!(grid.get_desired_position(Position::new(2, 0), 4) == Position::new(0, 0));
        grid.insert_tile(Position::new(0, 1), Position::new(0, 1), 2);
        assert!(grid.get_desired_position(Position::new(1, 1), 2) == Position::new(0, 1));
        grid.insert_tile(Position::new(0, 1), Position::new(0, 1), 4);
        assert!(grid.get_desired_position(Position::new(1, 1), 4) == Position::new(0, 1));
        // assert!(grid.get_new_xy(1, 1, 2) == (1, 1, 2));
        // assert!(grid.get_new_xy(3, 1, 4) == (0, 1, 8));
        // grid.insert_tile(1, 1, 2);
        // assert!(grid.get_new_xy(3, 1, 2) == (1, 1, 4));
    }

    // #[test]
    // fn one_tile_move_right() {
    //     let mut grid = Grid::new(10, Position::new(0, 0), 4);
    //     grid.insert_tile(2, 0, 2);
    //     grid.insert_tile(0, 0, 2);
    //     grid.on_tick(Some(Move::Right), &mut false);
    //     grid.on_tick(Some(Move::Left), &mut false);
    //     dbg!(&grid);
    //     // assert!(grid.get_tile(0, 0).unwrap().n == 4);
    //     // grid.insert_tile(3, 0, 4);
    //     // grid.on_tick(Some(Move::Right), &mut false);
    //     // dbg!(&grid);
    //     //
    //     assert!(false);
    // }

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
