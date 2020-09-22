// # main.rs
//
// Contains the cli and high-level orchestration of other components.

#![feature(exact_size_is_empty)]
#[macro_use]
extern crate log;

use std::io::Error;
use std::sync::{atomic::AtomicBool, atomic::Ordering, Arc};

use signal_hook::SIGTERM;

mod client;
mod controller;
mod state;
mod types;
mod ui;
mod views;

use crate::client::Client;
use crate::controller::Controller;
use crate::state::{ApplicationState, ApplicationStateInner};
use crate::ui::Ui;

fn main() -> Result<(), Error> {
    // Only enable the logging when compiling in debug mode. This makes the difference between
    // `info!` and `debug!` somewhat moot, so I'm just using them to switch between a 'normal'
    // amount of logging and 'excessive'.
    //
    if cfg!(debug_assertions) {
        let mut builder = env_logger::Builder::from_default_env();
        builder.target(env_logger::Target::Stderr).init();
    }

    info!("Starting...");

    // Keep track of whether we should exit (i.e., we got a sigterm)
    let should_terminate = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(SIGTERM, Arc::clone(&should_terminate))?;

    let client = Client::default();
    client.fetch_conversations();

    // The UI object has all of the cursive (rust tui library) logic.
    let mut ui = Ui::new();

    // State is a dumb model. The UI observer is called when state changes.
    let mut state = ApplicationStateInner::default();
    state.register_observer(Box::new(ui.observer.clone()));

    // The controller coordinates updates to the state and client fetches
    let mut controller = Controller::new(client, state, ui.executor.receiver.clone());

    // ## main render loop
    //
    // After handling any signals, progress the UI one 'frame' (step). This allows the UI to handle
    // any messages it got from channels, and also for the TUI library to process events and render a frame.
    while !should_terminate.load(Ordering::Relaxed) && ui.step() {
        controller.step();
    }

    Ok(())
}
