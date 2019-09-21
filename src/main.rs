#![warn(clippy::all, clippy::pedantic, clippy::nursery)]

#[macro_use]
extern crate clap;

mod activewindow;
mod args;
mod childpids;
mod operations;
mod pulseop;

use std::rc::Rc;

use operations::{Target, VolumeOp};

fn run() -> Option<()> {
    env_logger::from_env(env_logger::Env::default().default_filter_or("info"))
        .write_style(env_logger::WriteStyle::Auto)
        .default_format_module_path(false)
        .default_format_timestamp_nanos(true)
        .init();

    let arguments = args::get_arguments();
    let pid: u32 = arguments.pid.unwrap_or_else(|| {
        activewindow::active_window_pid(arguments.debug)
    });
    if arguments.debug {
        println!("op = {:?}", arguments.operation);
        println!("pid: {:?}", pid);
    }

    let traverse_children = arguments.traverse_children;
    let get_pids = move || {
        if traverse_children {
            Rc::new(childpids::get_children_pids(pid as i32))
        } else {
            Rc::new(vec![pid as i32])
        }
    };

    match arguments.operation {
        VolumeOp::ToggleMute => {
            match &arguments.target {
                Target::All => pulseop::run_pa_function(move |context, done| {
                    pulseop::perform_on_all_sinks(context, done, pulseop::op_toggle_mute);
                }),
                Target::Active => pulseop::run_pa_function(move |context, done| {
                    pulseop::perform_on_sinks_of_pids(context, done, get_pids(), pulseop::op_toggle_mute);
                }),
            }
        }
        VolumeOp::ChangeVolume(delta) => {
            match &arguments.target {
                Target::All => pulseop::run_pa_function(move |context, done| {
                    pulseop::perform_on_all_sinks(context, done, move |context, sinkinfo, callback|{
                        pulseop::op_change_volume_by(context, sinkinfo, delta, callback)
                    });
                }),
                Target::Active => pulseop::run_pa_function(move |context, done| {
                    pulseop::perform_on_sinks_of_pids(context, done, get_pids(), move |context, sinkinfo, callback|{
                        pulseop::op_change_volume_by(context, sinkinfo, delta, callback)
                    });
                }),
            }
        }
        VolumeOp::NoOp => pulseop::run_pa_function(move |context, done| {
            pulseop::perform_on_sinks_of_pids(context, done, Rc::new(Vec::new()), pulseop::op_noop);
        }),
        // VolumeOp::SetVolume(val) => {}
    };
    Some(())
}

fn main() {
    if run().is_none() {
        std::process::exit(1);
    }
}
