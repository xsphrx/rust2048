#![allow(unused_imports)]
#![allow(dead_code)]
mod draw;
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

use draw::{draw_number, draw_shape, get_bg_color_for_n, get_color_for_n, Direction};
use game::{Coordinates, Grid, Move, Position, Tile};
use std::fmt;
use std::rc::{Rc, Weak};
use std::sync::{Arc, Mutex, RwLock};

const BASE_TICK_RATE: u64 = 40;

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
            game_size: 4,
            animation_speed: 3,
            active_item: SettingsItem::GameSize,
        }
    }

    fn update_settings(&mut self, item: SettingsItem) {
        match item {
            SettingsItem::GameSize => {
                self.game_size = std::cmp::max((self.game_size + 1) % 9, 4);
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
    Play = 1,
    Reset = 2,
    Settings = 3,
    Exit = 4,
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
            2 => MenuItem::Reset,
            3 => MenuItem::Settings,
            4 => MenuItem::Exit,
            _ => MenuItem::Play,
        }
    }
}

pub enum InfoItem {
    GameLost,
    GameWon,
}

pub enum Screen {
    Menu(MenuItem),
    Game,
    Settings,
    Info(InfoItem),
}

fn main() -> Result<(), Box<dyn Error>> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let game = Grid::new(6, 4);
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
    let mut active_screen = Screen::Menu(MenuItem::Play);

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
        terminal.draw(|f| {
            // every screen should have a black background
            f.render_widget(
                Block::default().style(Style::default().bg(Color::Black)),
                f.size(),
            );
            match &active_screen {
                Screen::Menu(active_menu_item) => render_menu(f, active_menu_item),
                Screen::Game => {
                    let Rect {
                        width: terminal_width,
                        height: terminal_height,
                        ..
                    } = f.size();
                    match game.adjust_size(terminal_width, terminal_height) {
                        Ok(_) => render_game(f, &mut game),
                        Err(err) => render_error(f, err),
                    }
                }
                Screen::Settings => render_settings(f, settings.clone()),
                Screen::Info(info_item) => match info_item {
                    InfoItem::GameWon => render_info(f, "Game Won", "You have won the game!"),
                    InfoItem::GameLost => render_info(f, "Game Lost", "You have lost the game :("),
                },
            }
        })?;

        match rx.recv()? {
            Event::Input(event) => {
                if event.code == KeyCode::Char('q') {
                    disable_raw_mode()?;
                    terminal.show_cursor()?;
                    break;
                }
                match &active_screen {
                    Screen::Menu(active_menu_item) => match event.code {
                        KeyCode::Char('w') | KeyCode::Up => {
                            let item = *active_menu_item as u16 - 1;
                            active_screen = Screen::Menu(MenuItem::from(item));
                        }
                        KeyCode::Char('s') | KeyCode::Down => {
                            let item = *active_menu_item as u16 + 1;
                            active_screen = Screen::Menu(MenuItem::from(item));
                        }
                        KeyCode::Enter => match active_menu_item {
                            MenuItem::Play => {
                                active_screen = Screen::Game;
                            }
                            MenuItem::Reset => {
                                game = Grid::new(game.tile_width, game.size);
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
                                active_screen = Screen::Menu(MenuItem::Play);
                                continue;
                            }
                            KeyCode::Char('w') | KeyCode::Up => Some(Move::Up),
                            KeyCode::Char('s') | KeyCode::Down => Some(Move::Down),
                            KeyCode::Char('a') | KeyCode::Left => Some(Move::Left),
                            KeyCode::Char('d') | KeyCode::Right => Some(Move::Right),
                            _ => None,
                        };
                        game.on_tick(mv)
                            .expect("Error should've been caught earlier!");
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
                                SettingsItem::GameSize => {
                                    settings.update_settings(SettingsItem::GameSize);
                                    game.change_size(settings.game_size);
                                    game = Grid::new(game.tile_width, game.size);
                                }
                            },
                            KeyCode::Esc => {
                                active_screen = Screen::Menu(MenuItem::Play);
                            }
                            _ => (),
                        }
                    }
                    Screen::Info(_) => match event.code {
                        KeyCode::Enter => {
                            game = Grid::new(game.tile_width, game.size);
                            active_screen = Screen::Game;
                        }
                        KeyCode::Esc => active_screen = Screen::Menu(MenuItem::Play),
                        _ => (),
                    },
                }
            }
            Event::Tick => match &active_screen {
                Screen::Game => match game.on_tick(None) {
                    Err(err) if err == "Game Won" => {
                        active_screen = Screen::Info(InfoItem::GameWon)
                    }
                    Err(err) if err == "Game Lost" => {
                        active_screen = Screen::Info(InfoItem::GameLost)
                    }
                    _ => (),
                },
                _ => (),
            },
        }
    }

    Ok(())
}

pub fn render_menu<B>(f: &mut Frame<B>, active_item: &MenuItem)
where
    B: Backend,
{
    let chunks = Layout::default()
        .direction(tui::layout::Direction::Horizontal)
        .margin(2)
        .constraints([Constraint::Length(20), Constraint::Length(50)].as_ref())
        .split(f.size());

    let menu_text: Vec<Spans> = (1..=4)
        .map(|n| {
            let span;
            if *active_item as u16 == n {
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
    let menu = Paragraph::new(menu_text).block(Block::default());

    f.render_widget(menu, chunks[0]);
    render_controls(f, chunks[1]);

    let border = Block::default()
        .borders(Borders::ALL)
        .title("Menu")
        .border_type(BorderType::Plain);
    f.render_widget(border, f.size());
}

pub fn render_settings<B>(f: &mut Frame<B>, settings: Arc<RwLock<Settings>>)
where
    B: Backend,
{
    let settings = settings.read().unwrap();
    let text: Vec<Spans> = (1..=2)
        .map(|n| {
            let spans;
            if settings.active_item as u16 == n {
                spans = vec![
                    Span::styled(
                        SettingsItem::from(n).to_string(),
                        Style::default()
                            .fg(Color::LightBlue)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(
                        " ".to_string() + &settings.get_value(SettingsItem::from(n)).to_string(),
                    ),
                ];
            } else {
                spans = vec![
                    Span::raw(SettingsItem::from(n).to_string()),
                    Span::raw(
                        " ".to_string() + &settings.get_value(SettingsItem::from(n)).to_string(),
                    ),
                ];
            }
            Spans::from(spans)
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
    for (_, tile) in game.tiles.iter() {
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

    let rect = Rect {
        x: game.coordinates.x + game.width() + 5,
        y: 1,
        width: 25,
        height: game.height() - 1,
    };

    if rect.right() > f.size().right() || rect.bottom() > f.size().bottom() {
        // to make sure the controls don't go outside the terminal
        return;
    }

    render_controls(f, rect);
}

pub fn render_controls<B>(f: &mut Frame<B>, rect: Rect)
where
    B: Backend,
{
    let controls_text: Vec<Spans> = vec![
        Spans::from(vec![Span::raw("Controls")]),
        Spans::from(vec![Span::raw("Up - Arrow Up | W")]),
        Spans::from(vec![Span::raw("Down - Arrow Down | S")]),
        Spans::from(vec![Span::raw("Left - Arrow Left | A")]),
        Spans::from(vec![Span::raw("Right - Arrow Right | D")]),
        Spans::from(vec![Span::raw("Quit - Q")]),
        Spans::from(vec![Span::raw("Select - ENTER")]),
        Spans::from(vec![Span::raw("Back - ESC")]),
    ];
    let controls = Paragraph::new(controls_text)
        .block(Block::default().style(Style::default().fg(Color::DarkGray)));

    f.render_widget(controls, rect);
}

pub fn render_error<B>(f: &mut Frame<B>, error: String)
where
    B: Backend,
{
    let text = Paragraph::new(error)
        .style(Style::default().fg(Color::LightCyan))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::White))
                .title("Error")
                .border_type(BorderType::Plain),
        );
    f.render_widget(text, f.size());
}

pub fn render_info<B>(f: &mut Frame<B>, title: &str, message: &str)
where
    B: Backend,
{
    let size = f.size();
    let text: Vec<Spans> = vec![
        Spans::from(vec![Span::styled(
            message,
            Style::default()
                .fg(Color::LightBlue)
                .add_modifier(Modifier::BOLD),
        )]),
        Spans::from(vec![Span::raw("Press enter to reset and play again.")]),
    ];
    let info = Paragraph::new(text).block(
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::White))
            .title(title)
            .border_type(BorderType::Plain),
    );
    f.render_widget(info, size);
}
