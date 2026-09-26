//! Bar surface lifecycle: one layer surface per matching output.

use std::collections::{HashMap, VecDeque};
use std::ffi::c_void;
use std::ptr::NonNull;

use raw_window_handle::{WaylandDisplayHandle, WaylandWindowHandle};
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
    #[allow(dead_code, reason = "kept for output hotplug handling in M1")]
    output: Option<wl_output::WlOutput>,
    scale: i32,
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
    events: VecDeque<BarEvent>,
    pointer: Option<WlPointer>,
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
            events: VecDeque::new(),
            pointer: None,
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
    }

    /// Returns the Wayland surface for a surface id.
    #[must_use]
    pub fn wl_surface(&self, id: &ObjectId) -> Option<&wl_surface::WlSurface> {
        self.surfaces.get(id).map(|entry| entry.layer.wl_surface())
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

    fn new_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _seat: wl_seat::WlSeat) {}

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
    }

    fn remove_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _seat: wl_seat::WlSeat) {
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
            if !self.surfaces.contains_key(&id) {
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
