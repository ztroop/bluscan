use crossterm::event::{self, Event, KeyCode};
use ratatui::backend::Backend;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::TableState;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Row, Table},
    Terminal,
};
use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::structs::DeviceInfo;
use crate::utils::extract_manufacturer_data;

/// Displays the detected Bluetooth devices in a table and handles the user input.
/// The user can navigate the table, pause the scanning, and quit the application.
/// The detected devices are received through the provided `mpsc::Receiver`.
pub async fn viewer<B: Backend>(
    terminal: &mut Terminal<B>,
    mut rx: mpsc::Receiver<Vec<DeviceInfo>>,
    pause_signal: Arc<AtomicBool>,
) -> Result<(), Box<dyn Error>> {
    let mut table_state = TableState::default();
    table_state.select(Some(0));
    let mut devices = Vec::<DeviceInfo>::new();

    loop {
        // Draw UI
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints(
                    [
                        Constraint::Percentage(70),
                        Constraint::Percentage(20),
                        Constraint::Percentage(10),
                    ]
                    .as_ref(),
                )
                .split(f.size());

            let selected_style = Style::default().add_modifier(Modifier::REVERSED);
            let rows: Vec<Row> = devices
                .iter()
                .enumerate()
                .map(|(i, device)| {
                    let style = if table_state.selected() == Some(i) {
                        selected_style
                    } else {
                        Style::default()
                    };
                    let device_address = if device.address == "00:00:00:00:00:00" {
                        device.id.clone()
                    } else {
                        device.address.clone()
                    };
                    Row::new(vec![
                        device_address,
                        device.name.clone(),
                        device.tx_power.clone(),
                        device.rssi.clone(),
                    ])
                    .style(style)
                })
                .collect();

            let table = Table::new(
                rows,
                [
                    Constraint::Length(40),
                    Constraint::Length(30),
                    Constraint::Length(10),
                    Constraint::Length(10),
                ],
            )
            .header(
                Row::new(vec!["Address", "Name", "TX Power", "RSSI"])
                    .style(Style::default().fg(Color::Yellow)),
            )
            .block(
                Block::default()
                    .title("Detected Bluetooth Devices")
                    .borders(Borders::ALL),
            )
            .highlight_style(selected_style);

            f.render_stateful_widget(table, chunks[0], &mut table_state);

            // More details
            let more_detail_chunk = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(100)])
                .split(chunks[1]);
            let device_binding = DeviceInfo::default();
            let selected_device = devices
                .get(table_state.selected().unwrap_or(0))
                .unwrap_or(&device_binding);
            let services_binding = selected_device.services.len().to_string();
            let manufacturer_data = extract_manufacturer_data(&selected_device.manufacturer_data);
            let detail_table = Table::new(
                vec![
                    Row::new(vec!["Detected At:", &selected_device.detected_at]),
                    // get count of services
                    Row::new(vec!["Services:", &services_binding]),
                    Row::new(vec!["Company Code Identifier:", &manufacturer_data.0]),
                    Row::new(vec!["Manufacturer Data:", &manufacturer_data.1]),
                ],
                [Constraint::Length(30), Constraint::Length(70)],
            )
            .block(Block::default().title("More Detail").borders(Borders::ALL));
            f.render_widget(detail_table, more_detail_chunk[0]);

            // Info table
            let current_state = pause_signal.load(Ordering::SeqCst);
            let info_rows = vec![Row::new(vec![
                "[q → quit]",
                "[up/down → navigate]",
                if current_state {
                    "[s → start scanning]"
                } else {
                    "[s → stop scanning]"
                },
            ])
            .style(Style::default().fg(Color::DarkGray))];
            let info_table = Table::new(
                info_rows,
                [
                    Constraint::Length(10),
                    Constraint::Length(20),
                    Constraint::Length(20),
                ],
            )
            .column_spacing(1);
            let info_chunk = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(100)])
                .split(chunks[2]);
            f.render_widget(info_table, info_chunk[0]);
        })?;

        // Event handling
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('s') => {
                        let current_state = pause_signal.load(Ordering::SeqCst);
                        pause_signal.store(!current_state, Ordering::SeqCst);
                    }
                    KeyCode::Down => {
                        let next = match table_state.selected() {
                            Some(selected) => {
                                if selected >= devices.len() - 1 {
                                    0
                                } else {
                                    selected + 1
                                }
                            }
                            None => 0,
                        };
                        table_state.select(Some(next));
                    }
                    KeyCode::Up => {
                        let previous = match table_state.selected() {
                            Some(selected) => {
                                if selected == 0 {
                                    devices.len() - 1
                                } else {
                                    selected - 1
                                }
                            }
                            None => 0,
                        };
                        table_state.select(Some(previous));
                    }
                    _ => {}
                }
            }
        }

        // Check for new devices
        if let Ok(new_devices) = rx.try_recv() {
            devices = new_devices;
            if table_state.selected().is_none() {
                table_state.select(Some(0));
            }
        }
    }
    Ok(())
}
