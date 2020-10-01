// # main.rs
//
// Contains the cli and high-level orchestration of other components.

#![feature(exact_size_is_empty)]
#[macro_use]
extern crate log;

use tokio::time::{delay_for, Duration, Instant};

mod client;
mod controller;
mod state;
mod types;
mod ui;
mod views;

use crate::client::Client;
use crate::controller::Controller;
use crate::state::{ApplicationState, ApplicationStateInner};
use crate::ui::UiBuilder;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Only enable the logging when compiling in debug mode. This makes the difference between
    // `info!` and `debug!` somewhat moot, so I'm just using them to switch between a 'normal'
    // amount of logging and 'excessive'.
    //
    if cfg!(debug_assertions) {
        let mut builder = env_logger::Builder::from_default_env();
        builder.target(env_logger::Target::Stderr).init();
    }

    info!("Starting...");

    // The UI object has all of the cursive (rust tui library) logic.
    let (ui, ui_recv) = UiBuilder::new().build();
    let mut state = ApplicationStateInner::default();

    state.register_observer(Box::new(ui.clone()));
    let client = Client::new();
    let mut controller = Controller::new(client, state, ui_recv);

    controller.init().await?;

    tokio::select! {
        _ = controller.process_events() => {}
        _ = async {
            let mut next_frame = Instant::now() + Duration::from_millis(16);
            loop {
                let now = Instant::now();
                if now < next_frame {
                    delay_for(next_frame - now).await;
                }
                if !ui.borrow_mut().step() {
                    break
                }
                next_frame = Instant::now() + Duration::from_millis(16);

            }
        } => { info!("Exiting."); }
    }
    Ok(())
}
