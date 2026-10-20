//! Throwaway: the hello chart on ratatui's own Chart widget (braille marker,
//! line graph, user-given axis labels), with the same input loop as
//! hello-crossterm. Piped out, one frame rendered to a buffer and printed.
use bench_surfaces::{demo, parse};
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, MouseEventKind};
use ratatui::crossterm::execute;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::symbols::Marker;
use ratatui::widgets::{Axis, Chart, Dataset, GraphType};
use ratatui::{Frame, Terminal};
use std::io::{self, IsTerminal, Read};

fn draw(f: &mut Frame, points: &[(f64, f64)], lo: f64, hi: f64, cross: Option<u16>) {
    let n = points.len() as f64;
    let chart = Chart::new(vec![Dataset::default().marker(Marker::Braille).graph_type(GraphType::Line).style(Style::default().fg(Color::Cyan)).data(points)])
        .x_axis(Axis::default().bounds([0.0, n - 1.0]).labels(["0".to_string(), format!("{n}")]))
        .y_axis(Axis::default().bounds([lo, hi]).labels([format!("{lo:.1}"), format!("{:.1}", (lo + hi) / 2.0), format!("{hi:.1}")]));
    f.render_widget(chart, f.area());
    if let Some(x) = cross {
        let a = f.area();
        for y in a.top()..a.bottom().saturating_sub(2) {
            if let Some(cell) = f.buffer_mut().cell_mut((x, y)) {
                cell.set_fg(Color::Yellow);
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
    let points: Vec<(f64, f64)> = values.iter().enumerate().map(|(i, &v)| (i as f64, v)).collect();
    let (lo, hi) = values.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &v| (a.min(v), b.max(v)));
    if !io::stdout().is_terminal() {
        let mut t = Terminal::new(TestBackend::new(80, 24))?;
        t.draw(|f| draw(f, &points, lo, hi, None))?;
        let buf = t.backend().buffer().clone();
        let area: Rect = buf.area;
        for y in 0..area.height {
            let line: String = (0..area.width).map(|x| buf[(x, y)].symbol().to_string()).collect();
            println!("{line}");
        }
        return Ok(());
    }
    let mut terminal = ratatui::init();
    execute!(io::stdout(), EnableMouseCapture)?;
    let mut cross = None;
    loop {
        terminal.draw(|f| draw(f, &points, lo, hi, cross))?;
        match event::read()? {
            Event::Key(k) if matches!(k.code, KeyCode::Char('q') | KeyCode::Esc) => break,
            Event::Mouse(m) if matches!(m.kind, MouseEventKind::Moved | MouseEventKind::Drag(_)) => cross = Some(m.column),
            _ => {}
        }
    }
    execute!(io::stdout(), DisableMouseCapture)?;
    ratatui::restore();
    Ok(())
}
