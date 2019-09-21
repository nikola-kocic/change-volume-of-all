pub enum Target {
    All,
    Active,
}

#[derive(Debug)]
pub enum VolumeOp {
    ToggleMute,
    ChangeVolume(f64), // delta
    NoOp,
    // SetVolume(f64),
}
