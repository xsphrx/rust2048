#![allow(unused_imports)]
#![allow(dead_code)]
mod drawing;
mod game;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event as CEvent, KeyCode},
    execute, terminal,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::cell::RefCell;
use std::sync::mpsc::channel;
use std::thread;
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

use drawing::{draw_number, draw_shape, get_bg_color_for_n, get_color_for_n, Direction};
use game::{Coordinates, Grid, Move, Position, Tile};
use std::fmt;
use std::sync::{Arc, Mutex, RwLock};

const BASE_TICK_RATE: u64 = 20;

enum Event<I> {
    Input(I),
    Tick,
}

#[repr(u16)]
#[derive(Clone, Copy, Debug)]
pub enum SettingsItem {
    GameSize = 1,
    AnimationSpeed = 2,
}

impl fmt::Display for SettingsItem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl From<u16> for SettingsItem {
    fn from(n: u16) -> Self {
        match n {
            0 => SettingsItem::AnimationSpeed,
            2 => SettingsItem::AnimationSpeed,
            _ => SettingsItem::GameSize,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    game_size: u16,
    animation_speed: u16,
    active_item: SettingsItem,
}

impl Settings {
    fn new() -> Self {
        Self {
            game_size: 1,
            animation_speed: 3,
            active_item: SettingsItem::GameSize,
        }
    }

    fn update_settings(&mut self, item: SettingsItem) {
        match item {
            SettingsItem::GameSize => {
                self.game_size = std::cmp::max((self.game_size + 1) % 4, 1);
            }
            SettingsItem::AnimationSpeed => {
                self.animation_speed = std::cmp::max((self.animation_speed + 1) % 4, 1);
            }
        }
    }

    fn get_value(&self, item: SettingsItem) -> u16 {
        match item {
            SettingsItem::GameSize => self.game_size,
            SettingsItem::AnimationSpeed => self.animation_speed,
        }
    }
}

#[repr(u16)]
#[derive(Clone, Copy, Debug)]
pub enum MenuItem {
    Start = 1,
    Settings = 2,
    Exit = 3,
}

impl fmt::Display for MenuItem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl From<u16> for MenuItem {
    fn from(n: u16) -> Self {
        match n {
            0 => MenuItem::Exit,
            2 => MenuItem::Settings,
            3 => MenuItem::Exit,
            _ => MenuItem::Start,
        }
    }
}

enum Screen {
    Menu,
    Game,
    Settings,
}

fn main() -> Result<(), Box<dyn Error>> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let game = Grid::new(6, 6);
    let res = run_game(&mut terminal, game);

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

fn run_game<B: Backend>(
    terminal: &mut Terminal<B>,
    mut game: Grid,
) -> Result<(), Box<dyn std::error::Error>> {
    let settings = Arc::new(RwLock::new(Settings::new()));
    let settings_clone = settings.clone();
    let mut active_screen = Screen::Menu;
    let mut active_menu_item = MenuItem::Start;

    let (tx, rx) = channel();
    thread::spawn(move || {
        let mut last_tick = Instant::now();
        loop {
            let animation_speed = settings_clone.read().unwrap().animation_speed;
            let tick_rate = Duration::from_millis((4 - animation_speed) as u64 * BASE_TICK_RATE);
            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));

            if event::poll(timeout).expect("poll works") {
                if let CEvent::Key(key) = event::read().expect("can read events") {
                    tx.send(Event::Input(key)).expect("can send events");
                }
            }

            if last_tick.elapsed() >= tick_rate {
                if let Ok(_) = tx.send(Event::Tick) {
                    last_tick = Instant::now();
                }
            }
        }
    });

    loop {
        terminal.draw(|f| match active_screen {
            Screen::Menu => render_menu(f, active_menu_item),
            Screen::Game => render_game(f, &mut game),
            Screen::Settings => render_settings(f, settings.clone()),
        })?;

        match rx.recv()? {
            Event::Input(event) => {
                if event.code == KeyCode::Char('q') {
                    disable_raw_mode()?;
                    terminal.show_cursor()?;
                    break;
                }
                match active_screen {
                    Screen::Menu => match event.code {
                        KeyCode::Char('w') | KeyCode::Up => {
                            let item = active_menu_item as u16 - 1;
                            active_menu_item = MenuItem::from(item);
                        }
                        KeyCode::Char('s') | KeyCode::Down => {
                            let item = active_menu_item as u16 + 1;
                            active_menu_item = MenuItem::from(item);
                        }
                        KeyCode::Enter => match active_menu_item {
                            MenuItem::Start => {
                                active_screen = Screen::Game;
                            }
                            MenuItem::Settings => {
                                active_screen = Screen::Settings;
                            }
                            MenuItem::Exit => {
                                disable_raw_mode()?;
                                terminal.show_cursor()?;
                                break;
                            }
                        },
                        KeyCode::Esc => {
                            disable_raw_mode()?;
                            terminal.show_cursor()?;
                            break;
                        }
                        _ => (),
                    },
                    Screen::Game => {
                        let mv = match event.code {
                            KeyCode::Esc => {
                                active_screen = Screen::Menu;
                                continue;
                            }
                            KeyCode::Char('w') | KeyCode::Up => Some(Move::Up),
                            KeyCode::Char('s') | KeyCode::Down => Some(Move::Down),
                            KeyCode::Char('a') | KeyCode::Left => Some(Move::Left),
                            KeyCode::Char('d') | KeyCode::Right => Some(Move::Right),
                            _ => None,
                        };
                        game.on_tick(mv);
                    }
                    Screen::Settings => {
                        let mut settings = settings.write().unwrap();
                        match event.code {
                            KeyCode::Char('w') | KeyCode::Up => {
                                let item = settings.active_item as u16 - 1;
                                settings.active_item = SettingsItem::from(item);
                            }
                            KeyCode::Char('s') | KeyCode::Down => {
                                let item = settings.active_item as u16 + 1;
                                settings.active_item = SettingsItem::from(item);
                            }
                            KeyCode::Enter => match settings.active_item {
                                SettingsItem::AnimationSpeed => {
                                    settings.update_settings(SettingsItem::AnimationSpeed)
                                }
                                _ => (),
                            },
                            KeyCode::Esc => {
                                active_screen = Screen::Menu;
                            }
                            _ => (),
                        }
                    }
                }
            }
            Event::Tick => match active_screen {
                Screen::Game => {
                    game.on_tick(None);
                }
                _ => (),
            },
        }
    }

    Ok(())
}

pub fn render_menu<B>(f: &mut Frame<B>, active_item: MenuItem)
where
    B: Backend,
{
    let text: std::vec::Vec<tui::text::Spans> = (1..=3)
        .map(|n| {
            let span;
            if active_item as u16 == n {
                span = Span::styled(
                    MenuItem::from(n).to_string(),
                    Style::default()
                        .fg(Color::LightBlue)
                        .add_modifier(Modifier::BOLD),
                );
            } else {
                span = Span::raw(MenuItem::from(n).to_string())
            }
            Spans::from(vec![span])
        })
        .collect::<Vec<Spans>>();
    let menu = Paragraph::new(text).block(
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::White))
            .title("Menu")
            .border_type(BorderType::Plain),
    );

    f.render_widget(menu, f.size());
}

pub fn render_settings<B>(f: &mut Frame<B>, settings: Arc<RwLock<Settings>>)
where
    B: Backend,
{
    let settings = settings.read().unwrap();
    let text: std::vec::Vec<tui::text::Spans> = (1..=2)
        .map(|n| {
            let span;
            if settings.active_item as u16 == n {
                span = Span::styled(
                    SettingsItem::from(n).to_string()
                        + &settings.get_value(SettingsItem::from(n)).to_string(),
                    Style::default()
                        .fg(Color::LightBlue)
                        .add_modifier(Modifier::BOLD),
                );
            } else {
                span = Span::raw(
                    SettingsItem::from(n).to_string()
                        + &settings.get_value(SettingsItem::from(n)).to_string(),
                )
            }
            Spans::from(vec![span])
        })
        .collect::<Vec<Spans>>();
    let menu = Paragraph::new(text).block(
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::White))
            .title("Settings")
            .border_type(BorderType::Plain),
    );

    f.render_widget(menu, f.size());
}

pub fn render_game<B>(f: &mut Frame<B>, game: &mut Grid)
where
    B: Backend,
{
    let Rect {
        width: terminal_width,
        height: terminal_height,
        ..
    } = f.size();

    // get tile size that fits the screen
    let tile_sizes: [u16; 2] = [10, 6];
    let mut final_size: u16 = 0;
    for size in tile_sizes {
        let (width, height) = game.simulate_size(size);
        if width <= terminal_width && height <= terminal_height {
            final_size = size;
            break;
        }
    }

    if final_size == 0 {
        render_error(
            f,
            "The size of your terminal is too small and can't fit the game!",
        );
        return;
    }

    if game.tile_width != final_size {
        game.change_tile_size(final_size);
    }

    // render the grid
    let rect = Rect {
        x: game.coordinates.x,
        y: game.coordinates.y,
        width: game.width(),
        height: game.height(),
    };
    let block = Block::default()
        .title("2048")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);
    f.render_widget(block, rect);
    for x in 0..game.size {
        for y in 0..game.size {
            let Coordinates { x, y } = game.get_coordinates_at(Position::new(x, y));
            let rect = Rect {
                x,
                y,
                width: game.tile_width,
                height: game.tile_height,
            };
            let empty_tile = Block::default().style(Style::default().bg(Color::DarkGray));
            f.render_widget(empty_tile, rect);
        }
    }
    // render tiles
    for (pos, tile) in game.tiles.iter() {
        let rect = Rect {
            x: tile.coordinates.x,
            y: tile.coordinates.y,
            width: game.tile_width,
            height: game.tile_height,
        };
        let canvas = Canvas::default()
            .marker(symbols::Marker::Braille)
            .x_bounds([0.0, 10.0])
            .y_bounds([0.0, 10.0])
            .paint(|ctx| {
                draw_number(ctx, tile.n);
            });
        f.render_widget(canvas, rect);
        let tile = Block::default().style(Style::default().bg(get_bg_color_for_n(tile.n)));
        f.render_widget(tile, rect);
    }
}

pub fn render_error<B>(f: &mut Frame<B>, error: &str)
where
    B: Backend,
{
    let size = f.size();

    let error_message = Paragraph::new(error)
        .style(Style::default().fg(Color::LightCyan))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::White))
                .title("Error")
                .border_type(BorderType::Plain),
        );
    f.render_widget(error_message, size);
}
