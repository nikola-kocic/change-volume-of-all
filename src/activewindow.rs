use xcb;
use xcb_util::ewmh;

pub fn active_window_pid(debug: bool) -> u32 {
    let (xcb_con, screen_num) = xcb::Connection::connect(None).unwrap();
    let connection = ewmh::Connection::connect(xcb_con)
        .map_err(|(e, _)| e)
        .unwrap();
    let active_window: xcb::Window = ewmh::get_active_window(&connection, screen_num)
        .get_reply()
        .unwrap();
    let pid = ewmh::get_wm_pid(&connection, active_window)
        .get_reply()
        .unwrap();
    if debug {
        println!("active_window: {:X}", active_window);
    }
    pid
}
