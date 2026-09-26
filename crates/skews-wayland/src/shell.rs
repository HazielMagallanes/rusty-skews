//! Bar surface lifecycle: one layer surface per matching output.

use std::collections::{HashMap, VecDeque};
use std::ffi::c_void;
use std::ptr::NonNull;

use raw_window_handle::{WaylandDisplayHandle, WaylandWindowHandle};
use smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard;
use smithay_client_toolkit::reexports::client::protocol::wl_pointer::WlPointer;
use smithay_client_toolkit::reexports::client::protocol::{wl_output, wl_seat, wl_surface};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_registry,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        Capability, SeatHandler, SeatState,
        keyboard::{KeyEvent, KeyboardHandler, Keysym},
        pointer::{AxisScroll, PointerEvent, PointerEventKind, PointerHandler},
    },
    shell::{
        WaylandSurface,
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
    },
};

use crate::error::WaylandError;
use crate::{Connection, GlobalList, ObjectId, Proxy, QueueHandle};

/// Vertical gap between the bar and a dropdown panel.
pub const PANEL_GAP: i32 = 6;

/// Options controlling bar surface creation.
#[derive(Debug, Clone)]
pub struct BarOptions {
    /// Bar height in physical pixels.
    pub height: u32,
    /// Output name to bind to, or `*` for every output.
    pub monitor: String,
    /// Layer-shell namespace.
    pub namespace: String,
}

impl BarOptions {
    fn matches(&self, output_name: Option<&str>) -> bool {
        self.monitor == "*" || output_name == Some(self.monitor.as_str())
    }
}

/// Options for a dropdown panel surface.
#[derive(Debug, Clone)]
pub struct PanelOptions {
    /// Panel width in pixels.
    pub width: u32,
    /// Panel height in pixels.
    pub height: u32,
    /// Distance from the top edge (bar height plus a gap).
    pub margin_top: i32,
    /// Distance from the right edge.
    pub margin_right: i32,
    /// Layer-shell namespace.
    pub namespace: String,
}

/// Events emitted by [`BarShell`] for the runtime to react to.
#[derive(Debug)]
pub enum BarEvent {
    /// A layer surface was created for an output.
    Created {
        /// Surface object id, unique for the connection lifetime.
        id: ObjectId,
        /// Output name, when the compositor reports one.
        output: Option<String>,
        /// The Wayland surface to render into.
        surface: wl_surface::WlSurface,
        /// Integer scale factor of the output.
        scale: i32,
    },
    /// The compositor configured a surface with its final size.
    Configured {
        /// Surface object id.
        id: ObjectId,
        /// Width in surface-local coordinates.
        width: u32,
        /// Height in surface-local coordinates.
        height: u32,
    },
    /// A surface was closed by the compositor or its output disappeared.
    Closed {
        /// Surface object id.
        id: ObjectId,
    },
    /// Pointer input on one of the shell's surfaces.
    Pointer {
        /// Surface object id the pointer is over.
        id: ObjectId,
        /// Position in surface-local coordinates.
        position: (f64, f64),
        /// What the pointer did.
        kind: PointerKind,
    },
    /// A panel (and its backdrop) was created.
    PanelShown {
        /// Panel surface object id.
        id: ObjectId,
        /// Backdrop surface object id (clicks on it dismiss the panel).
        backdrop: ObjectId,
        /// Output the panel is bound to.
        output: Option<String>,
    },
    /// A panel was hidden.
    PanelHidden {
        /// Panel surface object id.
        id: ObjectId,
    },
    /// A popup (notification toast) was created.
    PopupShown {
        /// Popup surface object id.
        id: ObjectId,
        /// Output the popup is bound to.
        output: Option<String>,
    },
    /// A popup was hidden.
    PopupHidden {
        /// Popup surface object id.
        id: ObjectId,
    },
    /// A key was pressed while a shell surface had keyboard focus.
    KeyPressed {
        /// Raw keysym (`xkeysym` value).
        keysym: u32,
    },
}

/// Pointer interactions the runtime cares about.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PointerKind {
    /// The pointer moved.
    Motion,
    /// A button was pressed (Linux button codes, e.g. `0x110` = left).
    Press {
        /// The pressed button.
        button: u32,
    },
    /// A button was released.
    Release {
        /// The released button.
        button: u32,
    },
    /// Scroll wheel/touchpad motion, normalized to steps (one wheel click = 1.0).
    Scroll {
        /// Horizontal steps (positive = right).
        horizontal: f64,
        /// Vertical steps (positive = up).
        vertical: f64,
    },
}

struct SurfaceEntry {
    layer: LayerSurface,
    output: Option<wl_output::WlOutput>,
    scale: i32,
}

/// A dropdown panel plus its click-catching backdrop.
struct PanelEntry {
    layer: LayerSurface,
    #[allow(dead_code, reason = "held so the backdrop surface stays mapped")]
    backdrop: LayerSurface,
    backdrop_id: ObjectId,
    output_name: Option<String>,
}

/// A popup surface (notification toast): like a panel but without a backdrop.
struct PopupEntry {
    layer: LayerSurface,
    output_name: Option<String>,
}

/// Layer-surface state machine for the bar.
///
/// Feed it to a wayland-client [`EventQueue`](crate::EventQueue) and drain
/// [`BarEvent`]s after each dispatch.
pub struct BarShell {
    registry_state: RegistryState,
    output_state: OutputState,
    seat_state: SeatState,
    compositor: CompositorState,
    layer_shell: LayerShell,
    options: BarOptions,
    surfaces: HashMap<ObjectId, SurfaceEntry>,
    outputs: HashMap<String, wl_output::WlOutput>,
    panels: HashMap<ObjectId, PanelEntry>,
    popups: HashMap<ObjectId, PopupEntry>,
    events: VecDeque<BarEvent>,
    pointer: Option<WlPointer>,
    keyboard: Option<WlKeyboard>,
    seats: Vec<wl_seat::WlSeat>,
}

impl BarShell {
    /// Binds the required globals and creates the shell state.
    pub fn bind(
        globals: &GlobalList,
        qh: &QueueHandle<Self>,
        options: BarOptions,
    ) -> Result<Self, WaylandError> {
        let compositor =
            CompositorState::bind(globals, qh).map_err(|source| WaylandError::Bind {
                protocol: "wl_compositor",
                detail: source.to_string(),
            })?;

        let layer_shell = LayerShell::bind(globals, qh).map_err(|source| WaylandError::Bind {
            protocol: "zwlr_layer_shell_v1",
            detail: source.to_string(),
        })?;

        Ok(Self {
            registry_state: RegistryState::new(globals),
            output_state: OutputState::new(globals, qh),
            seat_state: SeatState::new(globals, qh),
            compositor,
            layer_shell,
            options,
            surfaces: HashMap::new(),
            outputs: HashMap::new(),
            panels: HashMap::new(),
            popups: HashMap::new(),
            events: VecDeque::new(),
            pointer: None,
            keyboard: None,
            seats: Vec::new(),
        })
    }

    /// Takes all pending events.
    pub fn drain_events(&mut self) -> Vec<BarEvent> {
        self.events.drain(..).collect()
    }

    /// Updates the bar height and exclusive zone on every surface.
    ///
    /// Surfaces are recommitted, so the compositor sends fresh configure
    /// events with the new size.
    pub fn set_bar_height(&mut self, height: u32) {
        self.options.height = height;
        for entry in self.surfaces.values() {
            entry.layer.set_size(0, height);
            entry.layer.set_exclusive_zone(height as i32);
            entry.layer.commit();
        }
        for entry in self.panels.values() {
            entry.layer.set_margin(height as i32 + PANEL_GAP, 0, 0, 0);
            entry.layer.commit();
        }
        for entry in self.popups.values() {
            entry.layer.set_margin(height as i32 + PANEL_GAP, 0, 0, 0);
            entry.layer.commit();
        }
    }

    /// Shows a dropdown panel (and its backdrop) on an output.
    ///
    /// Returns `(panel_id, backdrop_id)`. Only one panel is expected at a
    /// time; callers hide the previous one first.
    pub fn show_panel(
        &mut self,
        qh: &QueueHandle<Self>,
        output_name: Option<&str>,
        options: &PanelOptions,
    ) -> Option<(ObjectId, ObjectId)> {
        let output = output_name
            .and_then(|name| self.outputs.get(name))
            .or_else(|| self.outputs.values().next())
            .cloned()?;

        // Backdrop first, so the panel stacks above it.
        let backdrop_surface = self.compositor.create_surface(qh);
        let backdrop = self.layer_shell.create_layer_surface(
            qh,
            backdrop_surface.clone(),
            Layer::Overlay,
            Some("rusty-skews-panel-backdrop"),
            Some(&output),
        );
        backdrop.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
        backdrop.set_exclusive_zone(0);
        backdrop.set_keyboard_interactivity(KeyboardInteractivity::None);
        backdrop.commit();
        let backdrop_id = backdrop_surface.id();

        let panel_surface = self.compositor.create_surface(qh);
        let layer = self.layer_shell.create_layer_surface(
            qh,
            panel_surface.clone(),
            Layer::Overlay,
            Some(options.namespace.as_str()),
            Some(&output),
        );
        layer.set_anchor(Anchor::TOP | Anchor::RIGHT);
        layer.set_size(options.width, options.height);
        layer.set_margin(options.margin_top, options.margin_right, 0, 0);
        layer.set_exclusive_zone(0);
        layer.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
        layer.commit();

        let id = panel_surface.id();
        self.panels.insert(
            id.clone(),
            PanelEntry {
                layer,
                backdrop,
                backdrop_id: backdrop_id.clone(),
                output_name: output_name.map(str::to_owned),
            },
        );

        self.events.push_back(BarEvent::PanelShown {
            id: id.clone(),
            backdrop: backdrop_id.clone(),
            output: output_name.map(str::to_owned),
        });

        Some((id, backdrop_id))
    }

    /// Hides a panel and its backdrop.
    pub fn hide_panel(&mut self, id: &ObjectId) {
        if self.panels.remove(id).is_some() {
            self.events
                .push_back(BarEvent::PanelHidden { id: id.clone() });
        }
    }

    /// Shows a popup surface (no backdrop) on an output.
    pub fn show_popup(
        &mut self,
        qh: &QueueHandle<Self>,
        output_name: Option<&str>,
        options: &PanelOptions,
    ) -> Option<ObjectId> {
        let output = output_name
            .and_then(|name| self.outputs.get(name))
            .or_else(|| self.outputs.values().next())
            .cloned()?;

        let surface = self.compositor.create_surface(qh);
        let layer = self.layer_shell.create_layer_surface(
            qh,
            surface.clone(),
            Layer::Overlay,
            Some(options.namespace.as_str()),
            Some(&output),
        );
        layer.set_anchor(Anchor::TOP | Anchor::RIGHT);
        layer.set_size(options.width, options.height);
        layer.set_margin(options.margin_top, options.margin_right, 0, 0);
        layer.set_exclusive_zone(0);
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        layer.commit();

        let id = surface.id();
        self.popups.insert(
            id.clone(),
            PopupEntry {
                layer,
                output_name: output_name.map(str::to_owned),
            },
        );
        self.events.push_back(BarEvent::PopupShown {
            id: id.clone(),
            output: output_name.map(str::to_owned),
        });

        Some(id)
    }

    /// Hides a popup surface.
    pub fn hide_popup(&mut self, id: &ObjectId) {
        if self.popups.remove(id).is_some() {
            self.events
                .push_back(BarEvent::PopupHidden { id: id.clone() });
        }
    }

    /// Returns the output name a surface belongs to.
    #[must_use]
    pub fn surface_output(&self, id: &ObjectId) -> Option<String> {
        if let Some(entry) = self.surfaces.get(id) {
            return entry
                .output
                .as_ref()
                .and_then(|output| self.output_state.info(output))
                .and_then(|info| info.name.clone());
        }
        self.panels
            .get(id)
            .and_then(|entry| entry.output_name.clone())
    }

    fn owns_surface(&self, id: &ObjectId) -> bool {
        self.surfaces.contains_key(id)
            || self.panels.contains_key(id)
            || self.popups.contains_key(id)
            || self.panels.values().any(|entry| entry.backdrop_id == *id)
    }

    /// Returns the Wayland surface for a surface id (bars, panels, popups, backdrops).
    #[must_use]
    pub fn wl_surface(&self, id: &ObjectId) -> Option<&wl_surface::WlSurface> {
        if let Some(entry) = self.surfaces.get(id) {
            return Some(entry.layer.wl_surface());
        }
        if let Some(entry) = self.panels.get(id) {
            return Some(entry.layer.wl_surface());
        }
        if let Some(entry) = self.popups.get(id) {
            return Some(entry.layer.wl_surface());
        }
        self.panels
            .values()
            .find(|entry| entry.backdrop_id == *id)
            .map(|entry| entry.backdrop.wl_surface())
    }

    /// Returns the integer scale factor of the output a surface belongs to.
    #[must_use]
    pub fn scale(&self, id: &ObjectId) -> Option<i32> {
        self.surfaces.get(id).map(|entry| entry.scale)
    }

    fn create_surface(&mut self, qh: &QueueHandle<Self>, output: wl_output::WlOutput) {
        let info = self.output_state.info(&output);
        let name = info.as_ref().and_then(|info| info.name.clone());
        if !self.options.matches(name.as_deref()) {
            return;
        }
        let scale = info.as_ref().map_or(1, |info| info.scale_factor);

        let surface = self.compositor.create_surface(qh);
        let layer = self.layer_shell.create_layer_surface(
            qh,
            surface.clone(),
            Layer::Top,
            Some(self.options.namespace.as_str()),
            Some(&output),
        );

        layer.set_anchor(Anchor::TOP | Anchor::LEFT | Anchor::RIGHT);
        layer.set_size(0, self.options.height);
        layer.set_exclusive_zone(self.options.height as i32);
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        layer.commit();

        let id = surface.id();
        if let Some(name) = name.clone() {
            self.outputs.insert(name, output.clone());
        }
        self.surfaces.insert(
            id.clone(),
            SurfaceEntry {
                layer,
                output: Some(output),
                scale,
            },
        );
        self.events.push_back(BarEvent::Created {
            id,
            output: name,
            surface,
            scale,
        });
    }

    fn remove_surface(&mut self, id: ObjectId) {
        if self.surfaces.remove(&id).is_some() {
            self.events.push_back(BarEvent::Closed { id });
        }
    }
}

impl ProvidesRegistryState for BarShell {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers![OutputState, SeatState];
}

impl SeatHandler for BarShell {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, seat: wl_seat::WlSeat) {
        self.seats.push(seat);
    }

    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer && self.pointer.is_none() {
            match self.seat_state.get_pointer(qh, &seat) {
                Ok(pointer) => self.pointer = Some(pointer),
                Err(error) => tracing::warn!(%error, "failed to create the pointer"),
            }
        }
        if capability == Capability::Keyboard && self.keyboard.is_none() {
            match self.seat_state.get_keyboard(qh, &seat, None) {
                Ok(keyboard) => self.keyboard = Some(keyboard),
                Err(error) => tracing::warn!(%error, "failed to create the keyboard"),
            }
        }
    }

    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer
            && let Some(pointer) = self.pointer.take()
        {
            pointer.release();
        }
        if capability == Capability::Keyboard
            && let Some(keyboard) = self.keyboard.take()
        {
            keyboard.release();
        }
    }

    fn remove_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _seat: wl_seat::WlSeat) {
    }
}

impl KeyboardHandler for BarShell {
    fn enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _serial: u32,
        _raw: &[u32],
        _keysyms: &[Keysym],
    ) {
    }

    fn leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _serial: u32,
    ) {
    }

    fn press_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        self.events.push_back(BarEvent::KeyPressed {
            keysym: event.keysym.raw(),
        });
    }

    fn release_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &WlKeyboard,
        _serial: u32,
        _event: KeyEvent,
    ) {
    }

    fn repeat_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        // Treat key repeat like a fresh press (text editing uses it).
        self.events.push_back(BarEvent::KeyPressed {
            keysym: event.keysym.raw(),
        });
    }

    fn update_modifiers(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &WlKeyboard,
        _serial: u32,
        _modifiers: smithay_client_toolkit::seat::keyboard::Modifiers,
        _raw_modifiers: smithay_client_toolkit::seat::keyboard::RawModifiers,
        _layout: u32,
    ) {
    }

    fn update_repeat_info(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &WlKeyboard,
        _info: smithay_client_toolkit::seat::keyboard::RepeatInfo,
    ) {
    }
}

impl PointerHandler for BarShell {
    fn pointer_frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _pointer: &WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            let id = event.surface.id();
            if !self.owns_surface(&id) {
                continue;
            }

            let kind = match event.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    Some(PointerKind::Motion)
                }
                PointerEventKind::Leave { .. } => None,
                PointerEventKind::Press { button, .. } => Some(PointerKind::Press { button }),
                PointerEventKind::Release { button, .. } => Some(PointerKind::Release { button }),
                PointerEventKind::Axis {
                    horizontal,
                    vertical,
                    ..
                } => {
                    let steps = |axis: AxisScroll| -> f64 {
                        if axis.value120 != 0 {
                            f64::from(axis.value120) / 120.0
                        } else if axis.discrete != 0 {
                            f64::from(axis.discrete)
                        } else {
                            axis.absolute.signum() * f64::from(axis.absolute.abs() > 0.0)
                        }
                    };
                    Some(PointerKind::Scroll {
                        horizontal: steps(horizontal),
                        vertical: steps(vertical),
                    })
                }
            };

            if let Some(kind) = kind {
                self.events.push_back(BarEvent::Pointer {
                    id: id.clone(),
                    position: event.position,
                    kind,
                });
            }
        }
    }
}

impl OutputHandler for BarShell {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        self.create_surface(qh, output);
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        let id = output.id();
        let surface_ids: Vec<ObjectId> = self
            .surfaces
            .iter()
            .filter(|(_, entry)| {
                entry
                    .output
                    .as_ref()
                    .is_some_and(|stored| stored.id() == id)
            })
            .map(|(id, _)| id.clone())
            .collect();
        for surface_id in surface_ids {
            self.remove_surface(surface_id);
        }

        let panel_ids: Vec<ObjectId> = self
            .panels
            .iter()
            .filter(|(_, entry)| {
                entry
                    .output_name
                    .as_ref()
                    .and_then(|name| self.outputs.get(name))
                    .is_some_and(|stored| stored.id() == id)
            })
            .map(|(id, _)| id.clone())
            .collect();
        for panel_id in panel_ids {
            self.hide_panel(&panel_id);
        }

        let popup_ids: Vec<ObjectId> = self
            .popups
            .iter()
            .filter(|(_, entry)| {
                entry
                    .output_name
                    .as_ref()
                    .and_then(|name| self.outputs.get(name))
                    .is_some_and(|stored| stored.id() == id)
            })
            .map(|(id, _)| id.clone())
            .collect();
        for popup_id in popup_ids {
            self.hide_popup(&popup_id);
        }
    }
}

impl CompositorHandler for BarShell {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        new_factor: i32,
    ) {
        if let Some(entry) = self.surfaces.get_mut(&surface.id()) {
            entry.scale = new_factor;
        }
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}

impl LayerShellHandler for BarShell {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, layer: &LayerSurface) {
        let id = layer.wl_surface().id();

        if let Some(entry) = self.panels.remove(&id) {
            let _ = entry;
            self.events.push_back(BarEvent::PanelHidden { id });
            return;
        }
        if let Some(entry) = self.popups.remove(&id) {
            let _ = entry;
            self.events.push_back(BarEvent::PopupHidden { id });
            return;
        }
        self.remove_surface(id);
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        let id = layer.wl_surface().id();
        let (width, height) = configure.new_size;
        self.events
            .push_back(BarEvent::Configured { id, width, height });
    }
}

delegate_registry!(BarShell);
smithay_client_toolkit::delegate_dispatch2!(BarShell);

/// Creates a [`WaylandDisplayHandle`] for wgpu from a live connection.
///
/// The connection must outlive any surface created from the returned handle.
#[must_use]
pub fn display_handle(conn: &Connection) -> WaylandDisplayHandle {
    let ptr = NonNull::new(conn.backend().display_ptr() as *mut c_void)
        .expect("live Wayland connections have a display pointer");
    WaylandDisplayHandle::new(ptr)
}

/// Creates a [`WaylandWindowHandle`] for wgpu from a live `wl_surface`.
///
/// The surface must outlive any wgpu surface created from the returned handle.
#[must_use]
pub fn window_handle(surface: &wl_surface::WlSurface) -> WaylandWindowHandle {
    let ptr = NonNull::new(surface.id().as_ptr() as *mut c_void)
        .expect("live Wayland surfaces have a non-null id");
    WaylandWindowHandle::new(ptr)
}
