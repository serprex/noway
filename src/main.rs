#[macro_use] extern crate lazy_static;
extern crate rustwlc;

use std::sync::RwLock;
use std::cmp;
use std::process::Command;

use rustwlc::*;
use rustwlc::xkb::keysyms;

struct Compositor {
	pub view: Option<WlcView>,
	pub edges: ResizeEdge,
}

lazy_static! {
	static ref COMPOSITOR: RwLock<Compositor> =
		RwLock::new(Compositor { view: None, edges: ResizeEdge::empty() });
	static ref HIDDEN: RwLock<Vec<WlcView>> =
		RwLock::new(Vec::new());
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

fn start_interactive_resize(view: WlcView) {
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

fn update_layout(output: WlcOutput) {
	let resolution = output.get_resolution().unwrap();
	let views = output.get_views();
	if views.is_empty() { return }
	let hidden = HIDDEN.read().unwrap();
	let viewlen = views.len() - hidden.len();
	if viewlen == 1 {
		views[0].set_geometry(ResizeEdge::empty(), Geometry {
			origin: Point { x: 0, y: 0 },
			size: resolution,
		});
	} else {
		let w = resolution.w / 2;
		let h0 = resolution.h / ((viewlen + 1) / 2) as u32;
		let h1 = resolution.h / (viewlen / 2) as u32;
		for (i, view) in views.iter().filter(|v| !hidden.contains(v)).enumerate() {
			let (x, h) = if i&1 == 1 { (w as i32, h1) } else { (0, h0) };
			view.set_geometry(ResizeEdge::empty(), Geometry {
				origin: Point {
					x: x,
					y: h as i32 * (i / 2) as i32,
				},
				size: Size { w: w, h: h },
			});
		}
	}
}

extern fn on_output_resolution(output: WlcOutput, _from: &Size, _to: &Size) {
	update_layout(output);
}

extern fn on_view_created(view: WlcView) -> bool {
	view.set_mask(view.get_output().get_mask());
	view.bring_to_front();
	view.focus();
	update_layout(view.get_output());
	true
}

extern fn on_view_destroyed(view: WlcView) {
	let views = view.get_output().get_views();
	{
		let mut hidden = HIDDEN.write().unwrap();
		if let Some(&lastview) = views.iter().rev().filter(|v| !hidden.contains(v)).next() {
			lastview.focus();
		}
		if let Some(idx) = hidden.iter().position(|&v| v == view) {
			hidden.remove(idx);
		}
	}
	update_layout(view.get_output());
}

extern fn on_view_focus(view: WlcView, focused: bool) {
	view.set_state(VIEW_ACTIVATED, focused);
}

extern fn on_view_request_move(view: WlcView, _: &Point) {
	start_interactive_move(view);
}

extern fn on_view_request_resize(view: WlcView, _: ResizeEdge, _: &Point) {
	start_interactive_resize(view);
}

extern fn on_keyboard_key(view: WlcView, _time: u32, mods: &KeyboardModifiers, key: u32, state: KeyState) -> bool {
	let sym = input::keyboard::get_keysym_for_key(key, *mods);
	if state == KeyState::Pressed && mods.mods == MOD_ALT {
		match sym {
			keysyms::KEY_d => {
				if !view.is_root() {
					view.close();
				}
			}
			keysyms::KEY_Left => {
				if !view.is_root() {
					view.send_to_back();
					let mut views = view.get_output().get_views();
					{
						let hidden = HIDDEN.read().unwrap();
						views.retain(|v| !hidden.contains(v));
					}
					if views.len() < 2 { return true }
					views.last().unwrap().focus();
				}
			}
			keysyms::KEY_Right => {
				if !view.is_root() {
					let mut views = view.get_output().get_views();
					{
						let hidden = HIDDEN.read().unwrap();
						views.retain(|v| !hidden.contains(v));
					}
					if views.len() < 2 { return true }
					let first = views.first().unwrap();
					first.bring_to_front();
					first.focus();
				}
			}
			keysyms::KEY_Down => {
				if !view.is_root() {
					view.send_to_back();
					{
						let mut hidden = HIDDEN.write().unwrap();
						hidden.push(view);
					}
					update_layout(view.get_output());
				}
			}
			keysyms::KEY_Up => {
				if !view.is_root() {
					if let Some(hview) = {
						let mut hidden = HIDDEN.write().unwrap();
						hidden.pop()
					} {
						hview.bring_to_front();
						hview.focus();
						update_layout(view.get_output());
					}
				}
			}
			keysyms::KEY_o => {
				terminate();
			}
			keysyms::KEY_q => {
				Command::new("/usr/local/bin/wayst").spawn().expect("Error executing terminal");
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
					0x111 => start_interactive_resize(view),
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
	rustwlc::input::pointer::set_position(*point);
	let comp = COMPOSITOR.read().unwrap();
	if let Some(ref view) = comp.view {
		let mut geo = view.get_geometry().unwrap();
		if comp.edges.bits() != 0 {
			geo.size.w = if point.x > geo.origin.x { cmp::max(point.x - geo.origin.x, 32) as u32 } else { 32 };
			geo.size.h = if point.y > geo.origin.y { cmp::max(point.y - geo.origin.y, 32) as u32 } else { 32 };
		}
		else {
			geo.origin = *point;
		}
		view.set_geometry(comp.edges, geo);
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
	run_fn();
}

