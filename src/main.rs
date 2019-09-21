#![warn(clippy::all, clippy::pedantic, clippy::nursery)]

#[macro_use]
extern crate clap;

mod activewindow;
mod args;
mod childpids;
mod operations;
mod pulseop;

use std::convert::TryFrom;
use std::rc::Rc;

use operations::{Target, VolumeOp};

fn run() -> Option<()> {
    env_logger::from_env(env_logger::Env::default().default_filter_or("info"))
        .write_style(env_logger::WriteStyle::Auto)
        .default_format_module_path(false)
        .default_format_timestamp_nanos(true)
        .init();

    let arguments = args::get_arguments();

    let (arg_pid, traverse_children) = (arguments.pid, arguments.traverse_children);

    let get_pids = move || {
        let pid = i32::try_from(arg_pid.unwrap_or_else(activewindow::active_window_pid)).unwrap();
        if traverse_children {
            Rc::new(childpids::get_children_pids(pid))
        } else {
            Rc::new(vec![pid])
        }
    };

    match arguments.operation {
        VolumeOp::ToggleMute => match &arguments.target {
            Target::All => pulseop::run_pa_function(move |context, done| {
                pulseop::perform_on_all_sinks(context, done, pulseop::op_toggle_mute);
            }),
            Target::Active => pulseop::run_pa_function(move |context, done| {
                pulseop::perform_on_sinks_of_pids(
                    context,
                    done,
                    get_pids(),
                    pulseop::op_toggle_mute,
                );
            }),
        },
        VolumeOp::ChangeVolume(delta) => match &arguments.target {
            Target::All => pulseop::run_pa_function(move |context, done| {
                pulseop::perform_on_all_sinks(context, done, move |context, sinkinfo, callback| {
                    pulseop::op_change_volume_by(context, sinkinfo, delta, callback)
                });
            }),
            Target::Active => pulseop::run_pa_function(move |context, done| {
                pulseop::perform_on_sinks_of_pids(
                    context,
                    done,
                    get_pids(),
                    move |context, sinkinfo, callback| {
                        pulseop::op_change_volume_by(context, sinkinfo, delta, callback)
                    },
                );
            }),
        },
        VolumeOp::NoOp => pulseop::run_pa_function(move |context, done| {
            pulseop::perform_on_sinks_of_pids(context, done, get_pids(), pulseop::op_noop);
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
