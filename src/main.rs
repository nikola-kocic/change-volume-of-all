#![warn(clippy::all, clippy::pedantic, clippy::nursery)]

use libpulse_binding as pulse;

use std::cell::RefCell;
use std::convert::TryFrom;
use std::env;
use std::ops::Deref;
use std::rc::Rc;
use std::sync::atomic;

use log::{debug, error, warn};
use pulse::context::introspect::{SinkInfo, SinkInputInfo};
use pulse::context::Context;
use pulse::def::Retval;
use pulse::mainloop::standard::IterateResult;
use pulse::mainloop::standard::Mainloop;
use pulse::volume::ChannelVolumes;
use pulse::volume::Volume;

fn run_pa_function(f: &'static dyn Fn(Rc<RefCell<Context>>, Rc<atomic::AtomicBool>))
{
    let mainloop = Rc::new(RefCell::new(
        Mainloop::new().expect("Failed to create mainloop"),
    ));

    let context = Rc::new(RefCell::new(
        Context::new(mainloop.borrow().deref(), "ChangeVolumeOfAllContext")
            .expect("Failed to create new context"),
    ));

    let done = Rc::new(atomic::AtomicBool::new(false));

    {
        let context_ref = Rc::clone(&context);
        let done_ref = Rc::clone(&done);
        let mut c = context.borrow_mut();
        c.set_state_callback(Some(Box::new(move || {
            let state = unsafe { (*context_ref.as_ptr()).get_state() };
            match state {
                pulse::context::State::Ready => f(Rc::clone(&context_ref), Rc::clone(&done_ref)),
                pulse::context::State::Failed | pulse::context::State::Terminated => {
                    done_ref.store(true, atomic::Ordering::Relaxed);
                }
                _ => {}
            }
        })));
        c.connect(None, pulse::context::flags::NOFLAGS, None)
            .expect("Failed to connect context");
    }

    // Main loop
    while !done.load(atomic::Ordering::Relaxed) {
        match mainloop.borrow_mut().iterate(true) {
            IterateResult::Quit(_) | IterateResult::Err(_) => {
                error!("iterate state was not success, quitting...");
                return;
            }
            IterateResult::Success(_) => {}
        }
    }

    // Clean shutdown
    mainloop.borrow_mut().quit(Retval(0)); // uncertain whether this is necessary
}

#[derive(Debug)]
struct SinkProcessInfo {
    got_all_sinks: bool,
    sink_count: u8,
    processed_sink_count: u8,
}

impl SinkProcessInfo {
    const fn new() -> Self {
        Self {
            got_all_sinks: false,
            sink_count: 0,
            processed_sink_count: 0,
        }
    }

    fn check_if_done(&self, done: &atomic::AtomicBool) {
        if self.got_all_sinks && (self.processed_sink_count == self.sink_count) {
            done.store(true, atomic::Ordering::Relaxed);
        }
    }
}

type CallbackF = Box<dyn FnMut(bool) + 'static>;

trait VolumeManipulatible<'a> {
    fn volumes(&self) -> pulse::volume::ChannelVolumes;
    fn uses_db_volume(&self) -> bool;
    fn name(&self) -> &'a Option<std::borrow::Cow<'a, str>>;
    fn is_mute(&self) -> bool;
    fn set_mute(&self, mute: bool, context: &Context, callback: CallbackF);
    fn set_volumes(&self, volumes: &ChannelVolumes, context: &Context, callback: CallbackF);
}

impl<'a> VolumeManipulatible<'a> for &'a SinkInfo<'a> {
    fn volumes(&self) -> pulse::volume::ChannelVolumes {
        self.volume
    }

    fn uses_db_volume(&self) -> bool {
        self.flags & pulse::def::sink_flags::DECIBEL_VOLUME
            == pulse::def::sink_flags::DECIBEL_VOLUME
    }

    fn name(&self) -> &'a Option<std::borrow::Cow<'a, str>> {
        &self.name
    }

    fn is_mute(&self) -> bool {
        self.mute
    }

    fn set_volumes(&self, volumes: &ChannelVolumes, context: &Context, callback: CallbackF) {
        let mut introspect = context.introspect();
        introspect.set_sink_volume_by_index(self.index, volumes, Some(callback));
    }

    fn set_mute(&self, mute: bool, context: &Context, callback: CallbackF) {
        let mut introspect = context.introspect();
        introspect.set_sink_mute_by_index(self.index, mute, Some(callback));
    }
}

impl<'a> VolumeManipulatible<'a> for &'a SinkInputInfo<'a> {
    fn volumes(&self) -> pulse::volume::ChannelVolumes {
        self.volume
    }

    fn uses_db_volume(&self) -> bool {
        true
    }

    fn name(&self) -> &'a Option<std::borrow::Cow<'a, str>> {
        &self.name
    }

    fn is_mute(&self) -> bool {
        self.mute
    }

    fn set_volumes(&self, volumes: &ChannelVolumes, context: &Context, callback: CallbackF) {
        let mut introspect = context.introspect();
        introspect.set_sink_input_volume(self.index, volumes, Some(callback));
    }

    fn set_mute(&self, mute: bool, context: &Context, callback: CallbackF) {
        let mut introspect = context.introspect();
        introspect.set_sink_input_mute(self.index, mute, Some(callback));
    }
}

fn perform_on_all_sinks<F>(context: Rc<RefCell<Context>>, done: Rc<atomic::AtomicBool>, op: F)
where
    F: Fn(&Context, &dyn VolumeManipulatible, CallbackF) + 'static,
{
    let sink_process_info = Rc::new(RefCell::new(SinkProcessInfo::new()));

    let callback = {
        let sink_process_info = Rc::clone(&sink_process_info);
        let done = Rc::clone(&done);

        move |_success: bool| {
            let mut sink_process_info_borrow = sink_process_info.borrow_mut();
            sink_process_info_borrow.processed_sink_count += 1;
            debug!("callback {:?}", sink_process_info_borrow);
            sink_process_info_borrow.check_if_done(&done);
        }
    };

    let introspect = context.borrow().introspect();
    introspect.get_sink_info_list(move |e| match e {
        pulse::callbacks::ListResult::Item(sink) => {
            debug!("get_sink_info_list: Got sink {}", sink.index);
            sink_process_info.borrow_mut().sink_count += 1;
            op(&context.borrow(), &sink, Box::new(callback.clone()));
        }
        pulse::callbacks::ListResult::End => {
            debug!("get_sink_info_list: Got End");
            let mut sink_process_info_borrow = sink_process_info.borrow_mut();
            sink_process_info_borrow.got_all_sinks = true;
            sink_process_info_borrow.check_if_done(&done);
        }
        pulse::callbacks::ListResult::Error => {
            error!("get_sink_info_list: Got Error");
            done.store(true, atomic::Ordering::Relaxed);
        }
    });
}


fn perform_something<F>(context: Rc<RefCell<Context>>, done: Rc<atomic::AtomicBool>, op: F)
where
    F: Fn(&Context, &dyn VolumeManipulatible, CallbackF) + 'static,
{
    let sink_process_info = Rc::new(RefCell::new(SinkProcessInfo::new()));

    let callback = {
        let sink_process_info = Rc::clone(&sink_process_info);
        let done = Rc::clone(&done);

        move |_success| {
            let mut sink_process_info_borrow = sink_process_info.borrow_mut();
            sink_process_info_borrow.processed_sink_count += 1;
            debug!("callback {:?}", sink_process_info_borrow);
            sink_process_info_borrow.check_if_done(&done);
        }
    };

    let introspect = context.borrow().introspect();
    introspect.get_sink_input_info_list(move |e| match e {
        pulse::callbacks::ListResult::Item(sinkinputinfo) => {
            debug!("get_sink_input_info_list: Got sink {}", sinkinputinfo.index);
            sink_process_info.borrow_mut().sink_count += 1;
            let proplist = &sinkinputinfo.proplist;
            if let Some(pid_s) = proplist.get_str("application.process.id") {
                let pid = i32::from_str_radix(&pid_s, 10).unwrap();
                dbg!(pid);
            };
            callback(true);
            op(&context.borrow(), &sinkinputinfo, Box::new(move |_success| {

            }));
        }
        pulse::callbacks::ListResult::End => {
            debug!("get_sink_input_info_list: Got End");
            let mut sink_process_info_borrow = sink_process_info.borrow_mut();
            sink_process_info_borrow.got_all_sinks = true;
            sink_process_info_borrow.check_if_done(&done);
        }
        pulse::callbacks::ListResult::Error => {
            error!("get_sink_input_info_list: Got Error");
            done.store(true, atomic::Ordering::Relaxed);
        }
    });
}

fn volume_to_percent(volume: Volume) -> f64 {
    (f64::from(volume.0) * 100.0) / f64::from(pulse::volume::VOLUME_NORM.0)
}

fn percent_to_volume(percent: f64) -> Option<Volume> {
    if percent < 0.0 {
        Some(Volume::from(pulse::volume::DECIBEL_MINUS_INFINITY))
    } else {
        let volume_val = ((percent * f64::from(pulse::volume::VOLUME_NORM.0)) / 100.).round();
        if !volume_val.is_finite() || volume_val > i32::max_value().into() {
            error!("Value too large: {}", volume_val);
            return None;
        }
        #[allow(clippy::cast_possible_truncation)]
        let val = u32::try_from(volume_val as i32).unwrap();
        Some(Volume(val))
    }
}

fn modify_volumes_by_percent(sinkinfo: &dyn VolumeManipulatible, delta_percent: f64) -> Option<ChannelVolumes> {
    if !sinkinfo.uses_db_volume() {
        debug!("Sink {:?} does not use db volume", sinkinfo.name());
        return None;
    }

    let mut volumes = sinkinfo.volumes();
    for volume in volumes.get_mut() {
        let percent = volume_to_percent(*volume);
        debug!(
            "Current volume percent = {} for sink {:?}",
            percent, sinkinfo.name()
        );
        *volume = percent_to_volume(percent + delta_percent)?;
    }
    Some(volumes)
}

fn op_increase_volume(context: &Context, sinkinfo: &dyn VolumeManipulatible, mut callback: CallbackF) {
    if let Some(new_volumes) = modify_volumes_by_percent(sinkinfo, 5.0) {
        sinkinfo.set_volumes(&new_volumes, context, callback);
    } else {
        callback(false);
    }
}

fn op_decrease_volume(context: &Context, sinkinfo: &dyn VolumeManipulatible, mut callback: CallbackF) {
    if let Some(new_volumes) = modify_volumes_by_percent(sinkinfo, -5.0) {
        sinkinfo.set_volumes(&new_volumes, context, callback);
    } else {
        callback(false);
    }
}

fn op_toggle_mute(context: &Context, sinkinfo: &dyn VolumeManipulatible, callback: CallbackF) {
    sinkinfo.set_mute(!sinkinfo.is_mute(), context, callback);
}

fn op_noop(_context: &Context, sinkinfo: &dyn VolumeManipulatible, mut callback: CallbackF) {
    for volume in sinkinfo.volumes().get() {
        let percent = volume_to_percent(*volume);
        dbg!(percent);
        debug!(
            "Current volume percent = {} for sink {:?}",
            percent, sinkinfo.name()
        );
    }
    callback(true);
}

fn run() -> Option<()> {
    env_logger::from_env(env_logger::Env::default().default_filter_or("info"))
        .write_style(env_logger::WriteStyle::Auto)
        .default_format_module_path(false)
        .default_format_timestamp_nanos(true)
        .init();

    let mut args = env::args();
    if args.len() < 2 {
        eprintln!("Error: Missing argument: up, down or mute");
        return None;
    }
    let arg = args.nth(1).unwrap();

    let op: &'static dyn Fn(Rc<RefCell<Context>>, Rc<atomic::AtomicBool>) = match arg.as_ref() {
        "up" => &move |context, done| {
            perform_on_all_sinks(context, done, op_increase_volume);
        },
        "down" => &move |context, done| {
            perform_on_all_sinks(context, done, op_decrease_volume);
        },
        "mute" => &move |context, done| {
            perform_on_all_sinks(context, done, op_toggle_mute);
        },
        "noop" => &move |context, done| {
            perform_something(context, done, op_noop);
        },
        _ => {
            eprintln!("Error: Unknown argument value: {}", arg);
            return None;
        }
    };
    run_pa_function(op);
    Some(())
}

fn main() {
    if run().is_none() {
        std::process::exit(1);
    }
}
