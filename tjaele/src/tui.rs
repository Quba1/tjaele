use crate::app::{App, MonitorData};

mod events;
mod tui_blocks;

use anyhow::Result;
use events::EventHandler;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    DefaultTerminal, Frame,
};

use tui_blocks::{
    render_cooling_chart, render_fans_table, DeviceBlock, DriverBlock, ErrorBlock, SpecsBlock,
    TemperatureBlock, TimeBlock,
};

pub use events::Event;

#[derive(Debug)]
pub struct Tui {
    terminal: DefaultTerminal,
    pub events: EventHandler,
}

impl Tui {
    pub fn new(terminal: DefaultTerminal, tick_rate: f64) -> Self {
        Tui { terminal, events: EventHandler::new(tick_rate) }
    }

    pub fn draw(&mut self, app: &App) -> Result<()> {
        self.terminal.draw(|frame| Tui::draw_frame(frame, app))?;
        Ok(())
    }

    fn draw_frame(frame: &mut Frame, app: &App) {
        match &app.latest_data {
            Ok(data) => Tui::draw_normal_frame(frame, data),
            Err(err) => Tui::draw_error_frame(frame, err),
        }
    }

    fn draw_normal_frame(frame: &mut Frame, data: &MonitorData) {
        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Length(10), Constraint::Fill(1)])
            .split(frame.area());

        let upper_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![
                Constraint::Ratio(1, 3),
                Constraint::Ratio(1, 3),
                Constraint::Ratio(1, 3),
            ])
            .split(main_layout[0]);

        let lower_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(main_layout[1]);

        let cooler_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Length(4),
                Constraint::Length(data.gpu_state.persistent.num_fans as u16 + 3),
                Constraint::Fill(1),
            ])
            .split(lower_layout[0]);

        frame.render_widget(TimeBlock { data }, upper_layout[0]);
        frame.render_widget(DeviceBlock { data }, upper_layout[1]);
        frame.render_widget(DriverBlock { data }, upper_layout[2]);
        frame.render_widget(TemperatureBlock { data }, cooler_layout[0]);
        render_fans_table(frame, data, cooler_layout[1]);
        render_cooling_chart(frame, data, cooler_layout[2]);
        frame.render_widget(SpecsBlock { data }, lower_layout[1]);
    }

    fn draw_error_frame(frame: &mut Frame, error: &anyhow::Error) {
        frame.render_widget(ErrorBlock { error }, frame.area());
    }
}
