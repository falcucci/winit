use super::event;
use super::EventListenerHandle;
use crate::dpi::PhysicalPosition;
use crate::event::{Force, MouseButton};
use crate::keyboard::ModifiersState;

use event::ButtonsState;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::PointerEvent;

#[allow(dead_code)]
pub(super) struct PointerHandler {
    on_cursor_leave: Option<EventListenerHandle<dyn FnMut(PointerEvent)>>,
    on_cursor_enter: Option<EventListenerHandle<dyn FnMut(PointerEvent)>>,
    on_cursor_move: Option<EventListenerHandle<dyn FnMut(PointerEvent)>>,
    on_pointer_press: Option<EventListenerHandle<dyn FnMut(PointerEvent)>>,
    on_pointer_release: Option<EventListenerHandle<dyn FnMut(PointerEvent)>>,
    on_touch_cancel: Option<EventListenerHandle<dyn FnMut(PointerEvent)>>,
}

impl PointerHandler {
    pub fn new() -> Self {
        Self {
            on_cursor_leave: None,
            on_cursor_enter: None,
            on_cursor_move: None,
            on_pointer_press: None,
            on_pointer_release: None,
            on_touch_cancel: None,
        }
    }

    pub fn on_cursor_leave<F>(&mut self, canvas_common: &super::Common, mut handler: F)
    where
        F: 'static + FnMut(i32, ModifiersState),
    {
        self.on_cursor_leave = Some(canvas_common.add_event(
            "pointerout",
            move |event: PointerEvent| {
                // touch events are handled separately
                // handling them here would produce duplicate mouse events, inconsistent with
                // other platforms.
                if event.pointer_type() != "mouse" {
                    return;
                }

                handler(event.pointer_id(), event::mouse_modifiers(&event));
            },
        ));
    }

    pub fn on_cursor_enter<F>(&mut self, canvas_common: &super::Common, mut handler: F)
    where
        F: 'static + FnMut(i32, ModifiersState),
    {
        self.on_cursor_enter = Some(canvas_common.add_event(
            "pointerover",
            move |event: PointerEvent| {
                // touch events are handled separately
                // handling them here would produce duplicate mouse events, inconsistent with
                // other platforms.
                if event.pointer_type() != "mouse" {
                    return;
                }

                handler(event.pointer_id(), event::mouse_modifiers(&event));
            },
        ));
    }

    pub fn on_mouse_release<M, T>(
        &mut self,
        canvas_common: &super::Common,
        mut mouse_handler: M,
        mut touch_handler: T,
    ) where
        M: 'static + FnMut(i32, MouseButton, ModifiersState),
        T: 'static + FnMut(i32, PhysicalPosition<f64>, Force),
    {
        let canvas = canvas_common.raw.clone();
        self.on_pointer_release = Some(canvas_common.add_user_event(
            "pointerup",
            move |event: PointerEvent| {
                match event.pointer_type().as_str() {
                    "touch" => touch_handler(
                        event.pointer_id(),
                        event::touch_position(&event, &canvas)
                            .to_physical(super::super::scale_factor()),
                        Force::Normalized(event.pressure() as f64),
                    ),
                    "mouse" => mouse_handler(
                        event.pointer_id(),
                        event::mouse_button(&event).expect("no mouse button released"),
                        event::mouse_modifiers(&event),
                    ),
                    _ => (),
                }
            },
        ));
    }

    pub fn on_mouse_press<M, T>(
        &mut self,
        canvas_common: &super::Common,
        mut mouse_handler: M,
        mut touch_handler: T,
    ) where
        M: 'static + FnMut(i32, PhysicalPosition<f64>, MouseButton, ModifiersState),
        T: 'static + FnMut(i32, PhysicalPosition<f64>, Force),
    {
        let canvas = canvas_common.raw.clone();
        self.on_pointer_press = Some(canvas_common.add_user_event(
            "pointerdown",
            move |event: PointerEvent| {
                match event.pointer_type().as_str() {
                    "touch" => {
                        touch_handler(
                            event.pointer_id(),
                            event::touch_position(&event, &canvas)
                                .to_physical(super::super::scale_factor()),
                            Force::Normalized(event.pressure() as f64),
                        );
                    }
                    "mouse" => {
                        mouse_handler(
                            event.pointer_id(),
                            event::mouse_position(&event).to_physical(super::super::scale_factor()),
                            event::mouse_button(&event).expect("no mouse button pressed"),
                            event::mouse_modifiers(&event),
                        );

                        // Error is swallowed here since the error would occur every time the mouse is
                        // clicked when the cursor is grabbed, and there is probably not a situation where
                        // this could fail, that we care if it fails.
                        let _e = canvas.set_pointer_capture(event.pointer_id());
                    }
                    _ => (),
                }
            },
        ));
    }

    pub fn on_cursor_move<M, T>(
        &mut self,
        canvas_common: &super::Common,
        mut mouse_handler: M,
        mut touch_handler: T,
        prevent_default: bool,
    ) where
        M: 'static
            + FnMut(
                i32,
                PhysicalPosition<f64>,
                PhysicalPosition<f64>,
                ModifiersState,
                ButtonsState,
                Option<MouseButton>,
            ),
        T: 'static + FnMut(i32, PhysicalPosition<f64>, Force),
    {
        let canvas = canvas_common.raw.clone();
        self.on_cursor_move = Some(canvas_common.add_event(
            "pointermove",
            move |event: PointerEvent| {
                // coalesced events are not available on Safari
                #[wasm_bindgen]
                extern "C" {
                    #[wasm_bindgen(extends = PointerEvent)]
                    type PointerEventExt;

                    #[wasm_bindgen(method, getter, js_name = getCoalescedEvents)]
                    fn has_get_coalesced_events(this: &PointerEventExt) -> JsValue;
                }

                match event.pointer_type().as_str() {
                    "touch" => {
                        if prevent_default {
                            // prevent scroll on mobile web
                            event.prevent_default();
                        }
                    }
                    "mouse" => (),
                    _ => return,
                }

                let event: PointerEventExt = event.unchecked_into();

                let id = event.pointer_id();
                // cache buttons if the pointer is a mouse
                let mouse = (event.pointer_type() == "mouse").then(|| {
                    (
                        event::mouse_modifiers(&event),
                        event::mouse_buttons(&event),
                        event::mouse_button(&event),
                    )
                });

                // store coalesced events to extend it's lifetime
                let events = (!event.has_get_coalesced_events().is_undefined())
                    .then(|| event.get_coalesced_events())
                    // if coalesced events is empty, it's a chorded button event
                    .filter(|events| events.length() != 0);

                // make a single iterator depending on the availability of coalesced events
                let events = if let Some(events) = &events {
                    None.into_iter().chain(
                        Some(events.iter().map(PointerEventExt::unchecked_from_js))
                            .into_iter()
                            .flatten(),
                    )
                } else {
                    Some(event).into_iter().chain(None.into_iter().flatten())
                };

                for event in events {
                    // coalesced events should always have the same source as the root event
                    debug_assert_eq!(id, event.pointer_id());
                    debug_assert_eq!(mouse.is_none(), event.pointer_type() == "touch");

                    if let Some((modifiers, buttons, button)) = mouse {
                        // coalesced events should have the same buttons
                        debug_assert_eq!(modifiers, event::mouse_modifiers(&event));
                        debug_assert_eq!(buttons, event::mouse_buttons(&event));

                        mouse_handler(
                            id,
                            event::mouse_position(&event).to_physical(super::super::scale_factor()),
                            event::mouse_delta(&event).to_physical(super::super::scale_factor()),
                            modifiers,
                            buttons,
                            button,
                        );
                    } else {
                        touch_handler(
                            id,
                            event::touch_position(&event, &canvas)
                                .to_physical(super::super::scale_factor()),
                            Force::Normalized(event.pressure() as f64),
                        );
                    }
                }
            },
        ));
    }

    pub fn on_touch_cancel<F>(&mut self, canvas_common: &super::Common, mut handler: F)
    where
        F: 'static + FnMut(i32, PhysicalPosition<f64>, Force),
    {
        let canvas = canvas_common.raw.clone();
        self.on_touch_cancel = Some(canvas_common.add_event(
            "pointercancel",
            move |event: PointerEvent| {
                if event.pointer_type() == "touch" {
                    handler(
                        event.pointer_id(),
                        event::touch_position(&event, &canvas)
                            .to_physical(super::super::scale_factor()),
                        Force::Normalized(event.pressure() as f64),
                    );
                }
            },
        ));
    }

    pub fn remove_listeners(&mut self) {
        self.on_cursor_leave = None;
        self.on_cursor_enter = None;
        self.on_cursor_move = None;
        self.on_pointer_press = None;
        self.on_pointer_release = None;
        self.on_touch_cancel = None;
    }
}
