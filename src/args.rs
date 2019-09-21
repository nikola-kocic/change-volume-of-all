use crate::operations::{Target, VolumeOp};
use clap::{App, Arg, ArgGroup};

pub struct Arguments {
    pub operation: VolumeOp,
    pub target: Target,
    pub pid: Option<u32>,
}

pub fn get_arguments() -> Arguments {
    let matches = App::new("Change Volume of Active App")
        .version(crate_version!())
        .author(crate_authors!())
        .about(crate_description!())
        .arg(
            Arg::with_name("mute")
                .long("mute")
                .short("m")
                .help("Toggle mute")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("volume")
                .long("volume")
                .short("v")
                .help("Adjusts volume (in percent)")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("noop")
                .long("noop")
                .help("Noop")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("set_volume")
                .long("setvolume")
                .short("s")
                .help("Set volume to specified percent value")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("target")
                .long("target")
                .short("t")
                .help("Target all sinks or just active app")
                .takes_value(true)
                .possible_values(&["all", "active"]),
        )
        .arg(
            Arg::with_name("pid")
                .short("p")
                .long("pid")
                .help("Process ID to control, get active application if not specified")
                .takes_value(true),
        )
        .group(
            ArgGroup::with_name("operation")
                .args(&["mute", "volume", "set_volume", "noop"])
                .required(true),
        )
        .get_matches();
    let operation = {
        if matches.is_present("mute") {
            VolumeOp::ToggleMute
        } else if matches.is_present("noop") {
            VolumeOp::NoOp
        } else {
            let volume_present = matches.is_present("volume");
            if volume_present {
                let volume_delta_s: &str = matches.value_of("volume").unwrap();
                let volume_delta = volume_delta_s.parse::<f64>().unwrap();
                VolumeOp::ChangeVolume(volume_delta)
            } else {
                // let set_volume_present = matches.is_present("set_volume");
                // if set_volume_present {
                //     let set_volume_s: &str = matches.value_of("set_volume").unwrap();
                //     let set_volume = set_volume_s.parse::<f64>().unwrap();
                //     VolumeOp::SetVolume(set_volume)
                // } else {
                //     VolumeOp::ChangeVolume(0.0)
                // }
                panic!("Unsupported operation");
            }
        }
    };
    let target = {
        match matches.value_of("target").unwrap_or("all") {
            "all" => Target::All,
            "active" => Target::Active,
            _ => {
                panic!("Invalid target val");
            }
        }
    };

    let pid = {
        if matches.is_present("pid") {
            let pid_s: &str = matches.value_of("pid").unwrap();
            let pid_val = pid_s.parse::<u32>().unwrap();
            Some(pid_val)
        } else {
            None
        }
    };
    Arguments {
        pid,
        operation,
        target,
    }
}
