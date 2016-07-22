#[macro_use] extern crate lazy_static;
extern crate rustwlc;

use std::sync::RwLock;
use std::cmp;
use std::process::Command;

use rustwlc::*;
use rustwlc::xkb::keysyms;

struct Compositor {
    pub view: Option<WlcView>,
    pub edges: ResizeEdge
}

lazy_static! {
    static ref COMPOSITOR: RwLock<Compositor> =
        RwLock::new(Compositor { view: None, edges: ResizeEdge::empty() });
}

fn start_interactive_action(view: WlcView) -> bool {
    let mut comp = COMPOSITOR.write().unwrap();
    if comp.view != None {
        false
    } else {
        comp.view = Some(view);

        view.bring_to_front();
        true
    }
}

fn start_interactive_move(view: WlcView) {
    start_interactive_action(view);
}

fn start_interactive_resize(view: WlcView, _: ResizeEdge) {
    if start_interactive_action(view) {
        let mut comp = COMPOSITOR.write().unwrap();
        comp.edges = RESIZE_RIGHT | RESIZE_BOTTOM;
        view.set_state(VIEW_RESIZING, true);
    }
}

fn stop_interactive_action() {
    let mut comp = COMPOSITOR.write().unwrap();

    match comp.view {
        None => return,
        Some(ref view) =>
            view.set_state(VIEW_RESIZING, false)
    }

    comp.view = None;
    comp.edges = ResizeEdge::empty();
}

fn get_topmost_view(output: WlcOutput, offset: usize) -> Option<WlcView> {
    let views = output.get_views();
    if views.is_empty() { None }
    else {
        Some(views[(views.len() - 1 + offset) % views.len()])
    }
}

fn render_output(output: WlcOutput) {
    let resolution = output.get_resolution().unwrap();
    let views = output.get_views();
    if views.is_empty() { return }

    let mut toggle = false;
    let mut y = 0;
    let w = resolution.w / 2;
    let h = resolution.h / cmp::max((views.len() + 1) / 2, 1) as u32;
    for (i, view) in views.iter().enumerate() {
        view.set_geometry(ResizeEdge::empty(), Geometry {
            origin: Point { x: if toggle { w as i32 } else { 0 }, y: y },
            size: Size { w: if !toggle && i == views.len() - 1 { resolution.w } else { w }, h: h }
        });
        y += if toggle { h as i32 } else { 0 };
        toggle ^= true;
    }
}

extern fn on_output_resolution(output: WlcOutput, _from: &Size, _to: &Size) {
    render_output(output);
}

extern fn on_view_created(view: WlcView) -> bool {
    view.set_mask(view.get_output().get_mask());
    view.bring_to_front();
    view.focus();
    render_output(view.get_output());
    true
}

extern fn on_view_destroyed(view: WlcView) {
    if let Some(top_view) = get_topmost_view(view.get_output(), 0) {
        top_view.focus();
    }
    render_output(view.get_output());
}

extern fn on_view_focus(view: WlcView, focused: bool) {
    view.set_state(VIEW_ACTIVATED, focused);
}

extern fn on_view_request_move(view: WlcView, _: &Point) {
    start_interactive_move(view);
}

extern fn on_view_request_resize(view: WlcView, edges: ResizeEdge, _: &Point) {
    start_interactive_resize(view, edges);
}

extern fn on_keyboard_key(view: WlcView, _time: u32, mods: &KeyboardModifiers, key: u32, state: KeyState) -> bool {
    let sym = input::keyboard::get_keysym_for_key(key, mods.mods);
    if state == KeyState::Pressed && mods.mods == MOD_ALT {
        match sym {
            keysyms::KEY_d => {
                if !view.is_root() {
                    view.close();
                }
            },
            keysyms::KEY_Down => {
                view.send_to_back();
                get_topmost_view(view.get_output(), 0).unwrap().focus();
            },
            keysyms::KEY_o => {
                terminate();
            }
            keysyms::KEY_q => {
                Command::new("/usr/local/bin/wayst").spawn().expect("Error executing wayst");
            }
            _ => return false
        }
        true
    } else {
        false
    }
}

extern fn on_pointer_button(view: WlcView, _time: u32, mods: &KeyboardModifiers,
                            button: u32, state: ButtonState, _: &Point) -> bool {
    if state == ButtonState::Pressed {
        if !view.is_root() && mods.mods.contains(MOD_ALT) {
            view.focus();
            if mods.mods.contains(MOD_ALT) {
                match button {
                    0x110 => start_interactive_move(view),
                    0x111 => start_interactive_resize(view, ResizeEdge::empty()),
                    _ => (),
                }
            }
        }
    }
    else {
        stop_interactive_action();
    }

    let comp = COMPOSITOR.read().unwrap();
    comp.view.is_some()
}
extern fn on_pointer_motion(_in_view: WlcView, _time: u32, point: &Point) -> bool {
    rustwlc::input::pointer::set_position(point);
    let comp = COMPOSITOR.read().unwrap();
    if let Some(ref view) = comp.view {
        let mut geo = view.get_geometry().unwrap();
        if comp.edges.bits() != 0 {
            geo.size.w = cmp::max(point.x, 32) as u32;
            geo.size.h = cmp::max(point.y, 32) as u32;
            view.set_geometry(comp.edges, geo);
        }
        else {
            geo.origin = *point;
            view.set_geometry(ResizeEdge::empty(), geo);
        }
        true
    } else {
        false
    }
}

fn main() {
    callback::output_resolution(on_output_resolution);
    callback::view_created(on_view_created);
    callback::view_destroyed(on_view_destroyed);
    callback::view_focus(on_view_focus);
    callback::view_request_move(on_view_request_move);
    callback::view_request_resize(on_view_request_resize);
    callback::keyboard_key(on_keyboard_key);
    callback::pointer_button(on_pointer_button);
    callback::pointer_motion(on_pointer_motion);
    rustwlc::log_set_default_handler();
    let run_fn = rustwlc::init().expect("Unable to initialize wlc!");
    //Command::new("/usr/local/bin/wayst").spawn().expect("Error executing wayst");
    run_fn();
}

