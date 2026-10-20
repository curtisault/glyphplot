//! Throwaway: the hello chart on crossterm alone, as unicel's CLI is built.
//! Data from a file argument or stdin; piped out, one frame as text. In a
//! terminal: alternate screen, raw mode, mouse capture; the pointer moves a
//! crosshair, q or Esc leaves, a resize redraws.
use bench_surfaces::{chart, demo, parse, Grid};
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, MouseEventKind};
use crossterm::{execute, terminal};
use std::fmt::Write as _;
use std::io::{self, IsTerminal, Read, Write};

const INK: [(u8, u8, u8); 3] = [(150, 150, 150), (90, 200, 250), (250, 200, 90)];

fn write(grid: &Grid) -> String {
    let mut s = String::from("\x1b[H");
    for (y, row) in grid.cells.chunks(grid.cols).enumerate() {
        let _ = write!(s, "\x1b[{};1H", y + 1);
        let mut last = 255;
        for (x, &c) in row.iter().enumerate() {
            let k = grid.colors[y * grid.cols + x];
            if k != last {
                let (r, g, b) = INK[usize::from(k)];
                let _ = write!(s, "\x1b[38;2;{r};{g};{b}m");
                last = k;
            }
            s.push(char::from_u32(c).unwrap_or(' '));
        }
        s.push_str("\x1b[0m");
    }
    s
}

fn main() -> io::Result<()> {
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
    let mut out = io::stdout();
    terminal::enable_raw_mode()?;
    execute!(out, terminal::EnterAlternateScreen, EnableMouseCapture, crossterm::cursor::Hide)?;
    let (mut cols, mut rows) = terminal::size()?;
    let mut cross = None;
    loop {
        out.write_all(write(&chart(&values, usize::from(cols), usize::from(rows), cross)).as_bytes())?;
        out.flush()?;
        match event::read()? {
            Event::Key(k) if matches!(k.code, KeyCode::Char('q') | KeyCode::Esc) => break,
            Event::Mouse(m) if matches!(m.kind, MouseEventKind::Moved | MouseEventKind::Drag(_)) => cross = Some(usize::from(m.column)),
            Event::Resize(c, r) => (cols, rows) = (c, r),
            _ => {}
        }
    }
    execute!(out, DisableMouseCapture, crossterm::cursor::Show, terminal::LeaveAlternateScreen)?;
    terminal::disable_raw_mode()
}
