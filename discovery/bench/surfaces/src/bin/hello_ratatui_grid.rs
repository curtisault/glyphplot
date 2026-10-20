//! Throwaway: the hello chart's own grid shown through ratatui by a `Widget`
//! adapter (what a glyphplot ratatui adapter would do), with the same loop.
use bench_surfaces::{chart, demo, parse, Grid};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, MouseEventKind};
use ratatui::crossterm::execute;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::widgets::Widget;
use std::io::{self, IsTerminal, Read};

const INK: [Color; 3] = [Color::Rgb(150, 150, 150), Color::Rgb(90, 200, 250), Color::Rgb(250, 200, 90)];

/// A glyphplot frame as a ratatui widget: one `set_char` and `set_fg` a cell.
struct Plot<'a>(&'a Grid);

impl Widget for Plot<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let g = self.0;
        for y in 0..area.height.min(g.rows as u16) {
            for x in 0..area.width.min(g.cols as u16) {
                let i = usize::from(y) * g.cols + usize::from(x);
                if let Some(cell) = buf.cell_mut((area.x + x, area.y + y)) {
                    cell.set_char(char::from_u32(g.cells[i]).unwrap_or(' ')).set_fg(INK[usize::from(g.colors[i])]);
                }
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut text = String::new();
    if let Some(path) = std::env::args().nth(1) {
        text = std::fs::read_to_string(path)?;
    } else if !io::stdin().is_terminal() {
        io::stdin().read_to_string(&mut text)?;
    }
    let values = if text.trim().is_empty() { demo(200) } else { parse(&text) };
    if !io::stdout().is_terminal() {
        print!("{}", chart(&values, 80, 24, None).text());
        return Ok(());
    }
    let mut terminal = ratatui::init();
    execute!(io::stdout(), EnableMouseCapture)?;
    let mut cross = None;
    loop {
        terminal.draw(|f| {
            let a = f.area();
            let g = chart(&values, usize::from(a.width), usize::from(a.height), cross);
            f.render_widget(Plot(&g), a);
        })?;
        match event::read()? {
            Event::Key(k) if matches!(k.code, KeyCode::Char('q') | KeyCode::Esc) => break,
            Event::Mouse(m) if matches!(m.kind, MouseEventKind::Moved | MouseEventKind::Drag(_)) => cross = Some(usize::from(m.column)),
            _ => {}
        }
    }
    execute!(io::stdout(), DisableMouseCapture)?;
    ratatui::restore();
    Ok(())
}
