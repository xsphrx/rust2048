#![allow(unused_imports)]
#![allow(dead_code)]
mod drawing;
mod game;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event as CEvent, KeyCode},
    execute,
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

use drawing::{draw_number, draw_shape, Direction};
use game::{Coordinates, Grid, Move, Position, Tile};

enum Event<I> {
    Input(I),
    Tick,
}

fn main() -> Result<(), Box<dyn Error>> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let tick_rate = Duration::from_millis(10);
    let mut game = Grid::new(10, Coordinates::new(0, 0), 4);
    let pos = Position::new(1, 1);
    game.insert_tile(pos, 2);
    // game.insert_tile(2, 1, 2);

    let res = run_game(&mut terminal, game, tick_rate);

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
    tick_rate: Duration,
) -> Result<(), Box<dyn std::error::Error>> {
    let (tx, rx) = channel();
    let tick_rate = Duration::from_millis(50);
    thread::spawn(move || {
        let mut last_tick = Instant::now();
        loop {
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
        terminal.draw(|f| render(&game, f))?;
        let mut mv = None;
        let mut animating = false;

        match rx.recv()? {
            Event::Input(event) => match event.code {
                KeyCode::Char('q') => {
                    disable_raw_mode()?;
                    terminal.show_cursor()?;
                    break;
                }
                KeyCode::Char('a') => {
                    mv = Some(Move::Left);
                }
                KeyCode::Char('d') => {
                    mv = Some(Move::Right);
                }
                KeyCode::Char('s') => {
                    mv = Some(Move::Down);
                }
                KeyCode::Char('w') => {
                    mv = Some(Move::Up);
                }
                _ => {}
            },
            Event::Tick => {}
        }
        game.on_tick(mv, &mut animating);
    }

    Ok(())
}

pub fn render<B>(game: &Grid, f: &mut Frame<B>)
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
        let tile = Block::default().style(Style::default().bg(Color::DarkGray));
        f.render_widget(tile, rect);
    }
}
